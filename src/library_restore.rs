//! Bringing labels, projects, pins and notes back from the Zerkalo folder
//! (`library_export` wrote them there) into the library — on a new machine,
//! after moving the folder, or when another machine's changes arrive in it.
//!
//! The rule is **merge, never replace, never delete**:
//!
//! - project members are only ever *added*; a label is added when the other
//!   machine added it, and taken off a document only when the other machine
//!   took it off since the two last agreed — a label that was here and was
//!   never in their file is left alone;
//! - a note is never lost: if both sides have different notes, both are kept;
//! - a pin, an archive flag or a renamed title takes the folder's value only
//!   when it changed there since the library and the folder last agreed *and*
//!   hasn't been changed here — if both changed, what is here wins;
//! - nothing is created that the folder doesn't mention and nothing is
//!   deleted, anywhere;
//! - before the first change of a pass, a copy of the whole library is saved,
//!   so any merge can be undone by putting that file back.
//!
//! "Last agreed" is what `library_export` recorded for each file: when a file
//! has been merged, its content is recorded as agreed, so a later change to it
//! elsewhere can be told from one made here.
//!
//! Each machine's data is in its own folder under `.zerkalo/library/`. This
//! reads every folder *but its own* and changes none of them; the export writes
//! the combined result into this machine's folder only. Files left by an
//! earlier layout (directly under `.zerkalo/library/`) are read the same way.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::library::{DocState, Library, ProjectState};
use crate::library_export::DIR;

#[derive(Deserialize, Default, Debug, Clone, PartialEq)]
struct DocIn {
    #[serde(default)]
    labels: Vec<String>,
    #[serde(default)]
    pinned: bool,
    #[serde(default)]
    archived: bool,
    title: Option<String>,
    notes: Option<String>,
}

#[derive(Deserialize, Default, Debug)]
struct ProjectIn {
    name: String,
    root: Option<String>,
    #[serde(default)]
    documents: Vec<String>,
}

#[derive(Deserialize, Default, Debug)]
struct LabelIn {
    name: String,
    color: Option<String>,
}

#[derive(Deserialize, Default, Debug)]
struct LabelsIn {
    #[serde(default)]
    label: Vec<LabelIn>,
}

#[derive(Debug, Default, PartialEq)]
pub struct RestoreReport {
    /// Documents whose labels, pin, notes or title were changed.
    pub documents: usize,
    pub projects: usize,
    pub labels_created: usize,
    /// Documents archived because the other machine had archived them — they
    /// leave "All Documents", so the notice says so.
    pub archived: usize,
    /// Files that couldn't be read as what they should be; left untouched.
    pub unreadable: usize,
    /// Files naming a document the library doesn't have (yet). They are tried
    /// again next time rather than being given up on.
    pub waiting_for_document: usize,
    /// Whether a copy of the library was saved before changing it.
    pub backed_up: bool,
    /// The copy couldn't be saved, so nothing was changed.
    pub backup_failed: bool,
}

impl RestoreReport {
    pub fn changed_anything(&self) -> bool {
        self.documents + self.projects + self.labels_created > 0
    }
}

// ── planning: what would change, with nothing touched ──────────────────────

/// What to do to one document.
#[derive(Debug, Default, PartialEq)]
struct DocPlan {
    add_labels: Vec<String>,
    /// Labels the other machine took off since the two last agreed.
    remove_labels: Vec<String>,
    pinned: Option<bool>,
    archived: Option<bool>,
    title: Option<String>,
    notes: Option<String>,
}

impl DocPlan {
    fn is_empty(&self) -> bool {
        *self == DocPlan::default()
    }
}

/// A true/false that two machines can both change. With no record of an
/// earlier agreement (`base` is `None`) the only safe reading is to keep what
/// is here but let the folder switch a flag *on* — never off. With one, a
/// change made over there and not here is taken; a change made here wins.
fn merge_flag(ours: bool, theirs: bool, base: Option<bool>) -> Option<bool> {
    match base {
        None => (theirs && !ours).then_some(true),
        Some(b) if theirs != b && ours == b => Some(theirs),
        Some(_) => None,
    }
}

/// Notes are never overwritten and never dropped. Same or empty: nothing to
/// do. One containing the other: keep the longer. Otherwise both, one after
/// the other — which, when the other side later merges *this*, is recognised
/// as containing its own and settles instead of growing.
fn merge_notes(ours: Option<&str>, theirs: Option<&str>) -> Option<String> {
    let (ours, theirs) = (
        ours.map(str::trim).filter(|s| !s.is_empty()),
        theirs.map(str::trim).filter(|s| !s.is_empty()),
    );
    match (ours, theirs) {
        (_, None) => None,
        (None, Some(t)) => Some(t.to_string()),
        (Some(o), Some(t)) if o == t || o.contains(t) => None,
        (Some(o), Some(t)) if t.contains(o) => Some(t.to_string()),
        (Some(o), Some(t)) => Some(format!(
            "{o}\n\n\u{2014} from another machine \u{2014}\n{t}"
        )),
    }
}

fn plan_doc(ours: &DocState, theirs: &DocIn, base: Option<&DocIn>) -> DocPlan {
    let lower = |l: &str| l.trim().to_lowercase();
    let have: HashSet<String> = ours.labels.iter().map(|l| lower(l)).collect();
    let theirs_set: HashSet<String> = theirs.labels.iter().map(|l| lower(l)).collect();
    let base_set: Option<HashSet<String>> =
        base.map(|b| b.labels.iter().map(|l| lower(l)).collect());

    // Added: in their file, not here, and — when we have a record of what the
    // two last agreed — not something they already had then (a label that was
    // agreed and has since been taken off here must not come back just because
    // their file changed for another reason).
    let mut seen = HashSet::new();
    let add_labels = theirs
        .labels
        .iter()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .filter(|l| {
            let k = lower(l);
            !have.contains(&k)
                && base_set.as_ref().is_none_or(|b| !b.contains(&k))
                && seen.insert(k)
        })
        .map(String::from)
        .collect();
    // Removed: agreed once, gone from their file now, and still here. Needs a
    // record of the agreement — without one nothing can be called a removal.
    let remove_labels = match (&base_set, base) {
        (Some(b), Some(base)) => base
            .labels
            .iter()
            .map(|l| l.trim().to_string())
            .filter(|l| {
                let k = lower(l);
                b.contains(&k) && !theirs_set.contains(&k) && have.contains(&k)
            })
            .collect(),
        _ => Vec::new(),
    };

    // A renamed title: take theirs if it changed there and not here; with no
    // record, only fill in one that isn't set here.
    let title = match (&theirs.title, base) {
        (Some(t), None) if ours.title.is_none() => Some(t.clone()),
        (Some(t), Some(b)) if b.title.as_ref() != Some(t) && ours.title == b.title => {
            Some(t.clone())
        }
        _ => None,
    };

    DocPlan {
        add_labels,
        remove_labels,
        pinned: merge_flag(ours.pinned, theirs.pinned, base.map(|b| b.pinned)),
        archived: merge_flag(ours.archived, theirs.archived, base.map(|b| b.archived)),
        title,
        notes: merge_notes(ours.notes.as_deref(), theirs.notes.as_deref()),
    }
}

#[derive(Debug, Default, PartialEq)]
struct ProjectPlan {
    /// The project doesn't exist here yet.
    create: bool,
    /// Documents (as paths) to add at the end, in the folder's order.
    add: Vec<PathBuf>,
    root: Option<PathBuf>,
}

impl ProjectPlan {
    fn is_empty(&self) -> bool {
        *self == ProjectPlan::default()
    }
}

/// `path` for a name written relative to the folder, refusing anything that
/// would point outside it.
fn resolve(folder: &Path, rel: &str) -> Option<PathBuf> {
    let p = Path::new(rel);
    if p.components()
        .any(|c| !matches!(c, std::path::Component::Normal(_)))
    {
        return None;
    }
    Some(folder.join(p))
}

fn plan_project(
    ours: Option<&ProjectState>,
    theirs: &ProjectIn,
    folder: &Path,
    known: &HashSet<PathBuf>,
) -> ProjectPlan {
    let members: HashSet<&PathBuf> = ours
        .map(|p| p.documents.iter().collect())
        .unwrap_or_default();
    let mut add = Vec::new();
    for rel in &theirs.documents {
        if let Some(path) = resolve(folder, rel) {
            if known.contains(&path) && !members.contains(&path) && !add.contains(&path) {
                add.push(path);
            }
        }
    }
    let root = match (ours.and_then(|p| p.root.as_ref()), &theirs.root) {
        (None, Some(rel)) => resolve(folder, rel).filter(|p| known.contains(p)),
        _ => None,
    };
    ProjectPlan {
        create: ours.is_none(),
        add,
        root,
    }
}

// ── doing it ───────────────────────────────────────────────────────────────

/// Saves a copy of the library before the first change of a pass, keeping the
/// last few.
struct Backup<'a> {
    dir: Option<&'a Path>,
    done: bool,
    ok: bool,
}

impl Backup<'_> {
    /// Called before anything is changed. `false` means the safety copy could
    /// not be saved, and the caller must change nothing.
    fn before_change(&mut self, lib: &Library) -> bool {
        if self.done {
            return self.ok;
        }
        self.done = true;
        let Some(dir) = self.dir else { return true };
        std::fs::create_dir_all(dir).ok();
        let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
        let dest = dir.join(format!("library.before-restore-{stamp}.sqlite"));
        if lib.backup_to(&dest).is_err() {
            tracing::warn!("Couldn't save a copy of the library before restoring; not restoring");
            std::fs::remove_file(&dest).ok();
            self.ok = false;
            return false;
        }
        // Keep the newest five.
        let mut old: Vec<PathBuf> = std::fs::read_dir(dir)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.starts_with("library.before-restore-"))
            })
            .collect();
        old.sort();
        let excess = old.len().saturating_sub(5);
        for p in old.into_iter().take(excess) {
            std::fs::remove_file(p).ok();
        }
        true
    }
}

fn collect(dir: &Path, root: &Path, out: &mut Vec<(String, PathBuf)>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            collect(&p, root, out);
        } else if p.extension().is_some_and(|x| x == "toml") {
            if let Ok(rel) = p.strip_prefix(root) {
                let name = rel
                    .components()
                    .filter_map(|c| c.as_os_str().to_str())
                    .collect::<Vec<_>>()
                    .join("/");
                out.push((name, p));
            }
        }
    }
}

/// Something that tells you when the files may have changed — the count and
/// newest modification time under the export folder. Cheap enough to poll.
pub fn tree_stamp(folder: &Path) -> (usize, Option<std::time::SystemTime>) {
    let mut files = Vec::new();
    let root = folder.join(DIR);
    collect(&root, &root, &mut files);
    let newest = files
        .iter()
        .filter_map(|(_, p)| std::fs::metadata(p).and_then(|m| m.modified()).ok())
        .max();
    (files.len(), newest)
}

/// Merges whatever the folder holds that this library hasn't yet agreed to.
///
/// `backup_dir` is where the pre-change copy goes (`None` skips it, for tests
/// that don't want one).
pub fn restore(lib: &mut Library, folder: &Path, backup_dir: Option<&Path>) -> RestoreReport {
    let mut report = RestoreReport::default();
    let Ok(own) = lib.machine_id() else {
        return report;
    };
    let base = folder.join(DIR);

    // Every file in every folder but this machine's own: (the name it is
    // recorded under, its name within its folder, where it is).
    let mut files: Vec<(String, String, PathBuf)> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&base) {
        for e in entries.flatten() {
            let (path, id) = (e.path(), e.file_name().to_string_lossy().into_owned());
            if path.is_dir() && id != own && id != "docs" && id != "projects" {
                let mut found = Vec::new();
                collect(&path, &path, &mut found);
                files.extend(found.into_iter().map(|(n, p)| (format!("{id}/{n}"), n, p)));
            }
        }
    }
    // The earlier layout: files directly under the library folder.
    let mut legacy = Vec::new();
    collect(&base.join("docs"), &base, &mut legacy);
    collect(&base.join("projects"), &base, &mut legacy);
    if base.join("labels.toml").is_file() {
        legacy.push(("labels.toml".to_string(), base.join("labels.toml")));
    }
    files.extend(legacy.into_iter().map(|(n, p)| (n.clone(), n, p)));

    if files.is_empty() {
        return report;
    }
    // Labels first, so a document can use a colour that came with them.
    files.sort_by_key(|(key, name, _)| {
        let rank = match name.as_str() {
            "labels.toml" => 0,
            n if n.starts_with("docs/") => 1,
            _ => 2,
        };
        (rank, key.clone())
    });

    let records = lib.export_records().unwrap_or_default();
    let mut backup = Backup {
        dir: backup_dir,
        done: false,
        ok: true,
    };
    let Ok(snapshot) = lib.export_snapshot() else {
        return report;
    };
    let mut docs: HashMap<PathBuf, DocState> = snapshot
        .docs
        .iter()
        .map(|d| (d.path.clone(), d.clone()))
        .collect();
    let known: HashSet<PathBuf> = docs.keys().cloned().collect();
    let mut projects: HashMap<String, ProjectState> = snapshot
        .projects
        .iter()
        .map(|p| (p.name.to_lowercase(), p.clone()))
        .collect();
    let mut labels: HashMap<String, Option<String>> = snapshot
        .labels
        .iter()
        .map(|l| (l.name.to_lowercase(), l.color_hex.clone()))
        .collect();

    for (key, name, path) in files {
        let Ok(text) = std::fs::read_to_string(&path) else {
            report.unreadable += 1;
            continue;
        };
        let recorded = records.get(&key);
        // Already agreed: this is what the library last wrote or merged.
        if recorded == Some(&text) {
            continue;
        }
        let mut complete = true;

        if name == "labels.toml" {
            let Ok(theirs) = toml::from_str::<LabelsIn>(&text) else {
                report.unreadable += 1;
                continue;
            };
            for l in theirs.label.iter().filter(|l| !l.name.trim().is_empty()) {
                let key = l.name.trim().to_lowercase();
                match labels.get(&key) {
                    None => {
                        if !backup.before_change(lib) {
                            report.backup_failed = true;
                            return report;
                        }
                        if let Ok(id) = lib.create_label(&l.name) {
                            if let Some(c) = &l.color {
                                lib.set_label_color(id, Some(c)).ok();
                            }
                            labels.insert(key, l.color.clone());
                            report.labels_created += 1;
                        }
                    }
                    // A colour chosen here is never replaced; one that was
                    // left to Automatic can take the folder's.
                    Some(None) if l.color.is_some() => {
                        if let Some(existing) = lib
                            .all_labels()
                            .ok()
                            .and_then(|ls| ls.into_iter().find(|x| x.name.to_lowercase() == key))
                        {
                            if !backup.before_change(lib) {
                                report.backup_failed = true;
                                return report;
                            }
                            lib.set_label_color(existing.id, l.color.as_deref()).ok();
                            labels.insert(key, l.color.clone());
                        }
                    }
                    Some(_) => {}
                }
            }
        } else if let Some(rel) = name
            .strip_prefix("docs/")
            .and_then(|n| n.strip_suffix(".toml"))
        {
            let Ok(theirs) = toml::from_str::<DocIn>(&text) else {
                report.unreadable += 1;
                continue;
            };
            let Some(doc) = resolve(folder, rel).and_then(|p| docs.get(&p).cloned()) else {
                // Its document isn't here (yet): try again later.
                report.waiting_for_document += 1;
                continue;
            };
            let base = recorded.and_then(|r| toml::from_str::<DocIn>(r).ok());
            let plan = plan_doc(&doc, &theirs, base.as_ref());
            if !plan.is_empty() {
                if !backup.before_change(lib) {
                    report.backup_failed = true;
                    return report;
                }
                if !plan.add_labels.is_empty() {
                    let mut ids = Vec::new();
                    for l in &plan.add_labels {
                        let key = l.to_lowercase();
                        if let std::collections::hash_map::Entry::Vacant(slot) = labels.entry(key) {
                            slot.insert(None);
                            report.labels_created += 1;
                        }
                        if let Ok(id) = lib.create_label(l) {
                            ids.push(id);
                        }
                    }
                    lib.add_labels(&[doc.id], &ids).ok();
                }
                if !plan.remove_labels.is_empty() {
                    let ids: Vec<i64> = lib
                        .all_labels()
                        .unwrap_or_default()
                        .into_iter()
                        .filter(|l| {
                            plan.remove_labels
                                .iter()
                                .any(|r| r.eq_ignore_ascii_case(&l.name))
                        })
                        .map(|l| l.id)
                        .collect();
                    lib.remove_labels(&[doc.id], &ids).ok();
                }
                if let Some(p) = plan.pinned {
                    lib.set_pinned(doc.id, p).ok();
                }
                if let Some(a) = plan.archived {
                    lib.set_archived(doc.id, a).ok();
                    if a {
                        report.archived += 1;
                    }
                }
                if let Some(t) = &plan.title {
                    lib.set_title(doc.id, t).ok();
                }
                if let Some(n) = &plan.notes {
                    lib.set_notes(doc.id, Some(n)).ok();
                }
                report.documents += 1;
                // What is here now, for any later file about the same document.
                if let Some(d) = docs.get_mut(&doc.path) {
                    d.labels.extend(plan.add_labels.iter().cloned());
                    d.labels
                        .retain(|l| !plan.remove_labels.iter().any(|r| r.eq_ignore_ascii_case(l)));
                    d.pinned = plan.pinned.unwrap_or(d.pinned);
                    d.archived = plan.archived.unwrap_or(d.archived);
                    if plan.title.is_some() {
                        d.title = plan.title.clone();
                    }
                    if plan.notes.is_some() {
                        d.notes = plan.notes.clone();
                    }
                }
            }
        } else if name.starts_with("projects/") {
            let Ok(theirs) = toml::from_str::<ProjectIn>(&text) else {
                report.unreadable += 1;
                continue;
            };
            if theirs.name.trim().is_empty() {
                report.unreadable += 1;
                continue;
            }
            let ours = projects.get(&theirs.name.trim().to_lowercase());
            let plan = plan_project(ours, &theirs, folder, &known);
            // Members whose documents aren't here yet mean this file isn't
            // finished with.
            complete = theirs
                .documents
                .iter()
                .filter_map(|r| resolve(folder, r))
                .all(|p| known.contains(&p));
            if !plan.is_empty() {
                if !backup.before_change(lib) {
                    report.backup_failed = true;
                    return report;
                }
                let pid = match ours {
                    Some(_) => lib
                        .all_projects()
                        .ok()
                        .and_then(|ps| {
                            ps.into_iter().find(|p| {
                                p.name.to_lowercase() == theirs.name.trim().to_lowercase()
                            })
                        })
                        .map(|p| p.id),
                    None => lib.create_project(theirs.name.trim()).ok(),
                };
                if let Some(pid) = pid {
                    for path in &plan.add {
                        if let Some(d) = docs.get(path) {
                            lib.add_doc_to_project(pid, d.id).ok();
                        }
                    }
                    if let Some(r) = plan.root.as_ref().and_then(|p| docs.get(p)) {
                        lib.set_project_root(pid, Some(r.id)).ok();
                    }
                    report.projects += 1;
                    // Remember it, so another machine's file for the same
                    // project adds to it instead of making a second one.
                    let entry = projects
                        .entry(theirs.name.trim().to_lowercase())
                        .or_insert_with(|| ProjectState {
                            name: theirs.name.trim().to_string(),
                            root: None,
                            documents: Vec::new(),
                        });
                    entry.documents.extend(plan.add.iter().cloned());
                    if entry.root.is_none() {
                        entry.root = plan.root.clone();
                    }
                }
            }
        } else {
            // README.txt and anything else: not ours to interpret.
            continue;
        }

        if complete {
            // Agreed: from here the export may update this file with the merge.
            lib.set_export_record(&key, &text).ok();
        } else {
            report.waiting_for_document += 1;
        }
    }
    report.backed_up = backup.done;
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library_export::export;
    use tempfile::TempDir;

    fn machine() -> (Library, TempDir) {
        (Library::open_in_memory(), TempDir::new().unwrap())
    }

    fn doc(lib: &mut Library, folder: &TempDir, name: &str) -> i64 {
        let path = folder.path().join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "= Heading\nBody.\n").unwrap();
        lib.upsert_document(&path).unwrap()
    }

    fn lib_dir(folder: &TempDir) -> PathBuf {
        folder.path().join(DIR)
    }

    /// What `git pull` does: the other machine's files arrive over ours.
    fn pull(from: &TempDir, to: &TempDir) {
        fn walk(from: &Path, to: &Path) {
            std::fs::create_dir_all(to).unwrap();
            for e in std::fs::read_dir(from).unwrap().flatten() {
                let (src, dst) = (e.path(), to.join(e.file_name()));
                if src.is_dir() {
                    walk(&src, &dst);
                } else {
                    std::fs::copy(&src, &dst).unwrap();
                }
            }
        }
        walk(&lib_dir(from), &lib_dir(to));
    }

    /// Every file under the export folder, by name, with its content.
    fn snapshot_files(folder: &TempDir) -> std::collections::BTreeMap<String, String> {
        let root = lib_dir(folder);
        let mut files = Vec::new();
        fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
            if let Ok(rd) = std::fs::read_dir(dir) {
                for e in rd.flatten() {
                    let p = e.path();
                    if p.is_dir() {
                        walk(&p, out);
                    } else {
                        out.push(p);
                    }
                }
            }
        }
        walk(&root, &mut files);
        files
            .into_iter()
            .map(|p| {
                (
                    p.strip_prefix(&root)
                        .unwrap()
                        .to_string_lossy()
                        .into_owned(),
                    std::fs::read_to_string(&p).unwrap(),
                )
            })
            .collect()
    }

    /// The files a machine wrote, by name within its own folder.
    fn own_files(lib: &Library, folder: &TempDir) -> std::collections::BTreeMap<String, String> {
        let prefix = format!("{}/", lib.machine_id().unwrap());
        snapshot_files(folder)
            .into_iter()
            .filter_map(|(k, v)| k.strip_prefix(&prefix).map(|n| (n.to_string(), v)))
            .collect()
    }

    fn state(lib: &Library, folder: &TempDir, name: &str) -> DocState {
        let path = folder.path().join(name);
        lib.export_snapshot()
            .unwrap()
            .docs
            .into_iter()
            .find(|d| d.path == path)
            .unwrap_or_else(|| panic!("no {name}"))
    }

    // ── the planning rules, on their own ─────────────────────────────────

    fn ours() -> DocState {
        DocState {
            id: 1,
            path: PathBuf::from("/f/a.typ"),
            title: None,
            pinned: false,
            archived: false,
            notes: None,
            labels: vec![],
        }
    }

    #[test]
    fn a_flag_with_no_record_can_only_be_switched_on() {
        assert_eq!(merge_flag(false, true, None), Some(true));
        assert_eq!(
            merge_flag(true, false, None),
            None,
            "never switched off blind"
        );
        assert_eq!(merge_flag(true, true, None), None);
        assert_eq!(merge_flag(false, false, None), None);
    }

    #[test]
    fn a_flag_follows_a_change_made_elsewhere_unless_it_was_changed_here_too() {
        // They unpinned; we left it alone: follow.
        assert_eq!(merge_flag(true, false, Some(true)), Some(false));
        // They pinned; we left it: follow.
        assert_eq!(merge_flag(false, true, Some(false)), Some(true));
        // Both moved: ours stays.
        assert_eq!(merge_flag(false, false, Some(true)), None);
        // They didn't change it: ours stays, whatever it is.
        assert_eq!(merge_flag(true, true, Some(true)), None);
        assert_eq!(merge_flag(false, true, Some(true)), None);
    }

    #[test]
    fn notes_are_never_lost_and_settle_instead_of_growing() {
        assert_eq!(merge_notes(None, None), None);
        assert_eq!(merge_notes(Some("mine"), None), None);
        assert_eq!(merge_notes(None, Some("theirs")).as_deref(), Some("theirs"));
        assert_eq!(merge_notes(Some("same"), Some("same")), None);
        assert_eq!(
            merge_notes(Some("long and longer"), Some("long")),
            None,
            "ours holds theirs"
        );
        assert_eq!(
            merge_notes(Some("long"), Some("long and longer")).as_deref(),
            Some("long and longer"),
            "theirs holds ours"
        );
        let both = merge_notes(Some("mine"), Some("theirs")).unwrap();
        assert!(both.contains("mine") && both.contains("theirs"));
        // The other side merging the combined note back changes nothing more.
        assert_eq!(
            merge_notes(Some("theirs"), Some(&both)).as_deref(),
            Some(both.as_str())
        );
        assert_eq!(merge_notes(Some(&both), Some("theirs")), None);
        assert_eq!(merge_notes(Some(&both), Some("mine")), None);
    }

    #[test]
    fn with_no_record_labels_are_only_ever_added_and_matched_ignoring_case() {
        let mut mine = ours();
        mine.labels = vec!["Advent".into()];
        let theirs = DocIn {
            labels: vec!["advent".into(), "Lent".into(), "lent".into(), " ".into()],
            ..Default::default()
        };
        assert_eq!(plan_doc(&mine, &theirs, None).add_labels, vec!["Lent"]);
        // With no record of an earlier agreement, a file with *fewer* labels
        // can't be called a removal, and asks for nothing.
        let fewer = DocIn::default();
        assert!(plan_doc(&mine, &fewer, None).is_empty());
    }

    fn doc_in(labels: &[&str]) -> DocIn {
        DocIn {
            labels: labels.iter().map(|s| s.to_string()).collect(),
            ..Default::default()
        }
    }

    #[test]
    fn a_label_they_took_off_since_the_two_agreed_is_taken_off_here_too() {
        let mut mine = ours();
        mine.labels = vec!["Advent".into(), "Lent".into()];
        let base = doc_in(&["Advent", "Lent"]);
        let theirs = doc_in(&["Lent"]);
        let plan = plan_doc(&mine, &theirs, Some(&base));
        assert_eq!(plan.remove_labels, vec!["Advent"]);
        assert!(plan.add_labels.is_empty());
    }

    #[test]
    fn a_label_added_here_after_the_agreement_is_never_taken_away() {
        let mut mine = ours();
        mine.labels = vec!["Advent".into(), "MineOnly".into()];
        let base = doc_in(&["Advent"]);
        let theirs = doc_in(&[]); // they removed Advent
        let plan = plan_doc(&mine, &theirs, Some(&base));
        assert_eq!(plan.remove_labels, vec!["Advent"], "only what they removed");
    }

    #[test]
    fn a_label_taken_off_here_does_not_come_back_when_their_file_changes_for_another_reason() {
        let mine = ours(); // no labels: Advent was removed here
        let base = doc_in(&["Advent"]);
        let mut theirs = doc_in(&["Advent"]);
        theirs.pinned = true; // their file changed, but not its labels
        let plan = plan_doc(&mine, &theirs, Some(&base));
        assert!(plan.add_labels.is_empty(), "{plan:?}");
        assert_eq!(plan.pinned, Some(true));
    }

    #[test]
    fn a_label_they_added_is_added_here() {
        let base = doc_in(&["Advent"]);
        let theirs = doc_in(&["Advent", "New"]);
        let mut mine = ours();
        mine.labels = vec!["Advent".into()];
        assert_eq!(
            plan_doc(&mine, &theirs, Some(&base)).add_labels,
            vec!["New"]
        );
    }

    #[test]
    fn a_renamed_title_fills_a_gap_or_follows_an_unchallenged_change() {
        let theirs = DocIn {
            title: Some("Theirs".into()),
            ..Default::default()
        };
        assert_eq!(
            plan_doc(&ours(), &theirs, None).title.as_deref(),
            Some("Theirs")
        );
        let mut mine = ours();
        mine.title = Some("Mine".into());
        assert_eq!(
            plan_doc(&mine, &theirs, None).title,
            None,
            "never over a title chosen here"
        );
        let base = DocIn {
            title: Some("Old".into()),
            ..Default::default()
        };
        mine.title = Some("Old".into());
        assert_eq!(
            plan_doc(&mine, &theirs, Some(&base)).title.as_deref(),
            Some("Theirs")
        );
        mine.title = Some("Changed here".into());
        assert_eq!(plan_doc(&mine, &theirs, Some(&base)).title, None);
    }

    // ── a new machine ────────────────────────────────────────────────────

    /// Machine A with a bit of everything, exported.
    fn machine_a() -> (Library, TempDir) {
        let (mut lib, folder) = machine();
        let a = doc(&mut lib, &folder, "Thesis/main.typ");
        let b = doc(&mut lib, &folder, "Thesis/ch1.typ");
        let c = doc(&mut lib, &folder, "sermon.typ");
        let advent = lib.create_label("Advent").unwrap();
        let sermons = lib.create_label("Sermons").unwrap();
        lib.set_label_color(sermons, Some("#33d17a")).unwrap();
        lib.add_labels(&[c], &[advent, sermons]).unwrap();
        lib.set_pinned(c, true).unwrap();
        lib.set_archived(b, true).unwrap();
        lib.set_title(c, "Advent 2").unwrap();
        lib.set_notes(c, Some("Check the Isaiah reading")).unwrap();
        let p = lib.create_project("Thesis").unwrap();
        lib.add_doc_to_project(p, b).unwrap();
        lib.add_doc_to_project(p, a).unwrap();
        lib.set_project_root(p, Some(a)).unwrap();
        assert!(export(&lib, folder.path(), true, true).errors.is_empty());
        (lib, folder)
    }

    /// Machine B: same documents on disk and registered, nothing else.
    fn machine_b() -> (Library, TempDir) {
        let (mut lib, folder) = machine();
        for n in ["Thesis/main.typ", "Thesis/ch1.typ", "sermon.typ"] {
            doc(&mut lib, &folder, n);
        }
        (lib, folder)
    }

    #[test]
    fn a_new_machine_gets_back_labels_colours_pins_titles_notes_and_projects() {
        let (_a, fa) = machine_a();
        let (mut b, fb) = machine_b();
        pull(&fa, &fb);
        let r = restore(&mut b, fb.path(), None);

        let s = state(&b, &fb, "sermon.typ");
        assert_eq!(s.labels, vec!["Advent", "Sermons"]);
        assert!(s.pinned);
        assert_eq!(s.title.as_deref(), Some("Advent 2"));
        assert_eq!(s.notes.as_deref(), Some("Check the Isaiah reading"));
        assert!(state(&b, &fb, "Thesis/ch1.typ").archived);
        let colours: Vec<_> = b
            .all_labels()
            .unwrap()
            .into_iter()
            .map(|l| (l.name, l.color_hex))
            .collect();
        assert_eq!(
            colours,
            vec![
                ("Advent".to_string(), None),
                ("Sermons".to_string(), Some("#33d17a".to_string()))
            ]
        );
        let snap = b.export_snapshot().unwrap();
        assert_eq!(snap.projects.len(), 1);
        let p = &snap.projects[0];
        assert_eq!(p.name, "Thesis");
        assert_eq!(
            p.documents,
            vec![
                fb.path().join("Thesis/ch1.typ"),
                fb.path().join("Thesis/main.typ")
            ],
            "in the project's own order"
        );
        assert_eq!(p.root, Some(fb.path().join("Thesis/main.typ")));
        assert_eq!(r.unreadable, 0);
        assert_eq!(r.waiting_for_document, 0);
        assert!(r.changed_anything());
    }

    #[test]
    fn after_restoring_and_exporting_the_new_machine_agrees_with_the_old_byte_for_byte() {
        let (a, fa) = machine_a();
        let (mut b, fb) = machine_b();
        pull(&fa, &fb);
        restore(&mut b, fb.path(), None);
        let report = export(&b, fb.path(), true, true);
        assert_eq!(report.left_alone, 0, "nothing is stuck waiting");
        assert_eq!(own_files(&b, &fb), own_files(&a, &fa));
    }

    #[test]
    fn restoring_again_changes_nothing_and_takes_no_backup() {
        let (_a, fa) = machine_a();
        let (mut b, fb) = machine_b();
        let backups = TempDir::new().unwrap();
        pull(&fa, &fb);
        let first = restore(&mut b, fb.path(), Some(backups.path()));
        assert!(first.changed_anything() && first.backed_up);
        export(&b, fb.path(), true, true);
        let before = b.export_snapshot().unwrap().docs;

        let second = restore(&mut b, fb.path(), Some(backups.path()));
        assert!(!second.changed_anything());
        assert!(!second.backed_up);
        assert_eq!(b.export_snapshot().unwrap().docs, before);
        assert_eq!(std::fs::read_dir(backups.path()).unwrap().count(), 1);
    }

    #[test]
    fn restore_never_writes_to_the_folder_itself() {
        let (_a, fa) = machine_a();
        let (mut b, fb) = machine_b();
        pull(&fa, &fb);
        let docs_before = std::fs::read_to_string(fb.path().join("sermon.typ")).unwrap();
        let files_before = snapshot_files(&fb);
        restore(&mut b, fb.path(), None);
        assert_eq!(snapshot_files(&fb), files_before);
        assert_eq!(
            std::fs::read_to_string(fb.path().join("sermon.typ")).unwrap(),
            docs_before
        );
    }

    // ── the backup ───────────────────────────────────────────────────────

    #[test]
    fn a_copy_of_the_library_as_it_was_is_saved_before_the_first_change() {
        let (_a, fa) = machine_a();
        let (mut b, fb) = machine_b();
        let keep = b.create_label("Mine").unwrap();
        let _ = keep;
        let backups = TempDir::new().unwrap();
        pull(&fa, &fb);
        let r = restore(&mut b, fb.path(), Some(backups.path()));
        assert!(r.backed_up);

        let file = std::fs::read_dir(backups.path())
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        assert!(file
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("library.before-restore-"));
        let old = rusqlite::Connection::open(&file).unwrap();
        let names: Vec<String> = old
            .prepare("SELECT name FROM labels ORDER BY name")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        assert_eq!(
            names,
            vec!["Mine"],
            "the copy is the library before the merge"
        );
    }

    #[test]
    fn only_the_newest_five_copies_are_kept() {
        let (_a, fa) = machine_a();
        let (mut b, fb) = machine_b();
        let backups = TempDir::new().unwrap();
        for i in 0..6 {
            std::fs::write(
                backups
                    .path()
                    .join(format!("library.before-restore-2000010{i}-000000.sqlite")),
                "old",
            )
            .unwrap();
        }
        std::fs::write(backups.path().join("unrelated.txt"), "x").unwrap();
        pull(&fa, &fb);
        restore(&mut b, fb.path(), Some(backups.path()));
        let names: Vec<String> = std::fs::read_dir(backups.path())
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            names
                .iter()
                .filter(|n| n.starts_with("library.before-restore-"))
                .count(),
            5
        );
        assert!(
            names.contains(&"unrelated.txt".to_string()),
            "other files are not ours to prune"
        );
    }

    // ── what is here is never lost ───────────────────────────────────────

    #[test]
    fn local_labels_notes_and_pins_survive_a_merge_untouched() {
        let (_a, fa) = machine_a();
        let (mut b, fb) = machine_b();
        let id = doc(&mut b, &fb, "sermon.typ");
        let mine = b.create_label("Mine").unwrap();
        b.add_labels(&[id], &[mine]).unwrap();
        b.set_notes(id, Some("my own note")).unwrap();
        b.set_title(id, "My title").unwrap();
        pull(&fa, &fb);
        restore(&mut b, fb.path(), None);

        let s = state(&b, &fb, "sermon.typ");
        assert_eq!(
            s.labels,
            vec!["Advent", "Mine", "Sermons"],
            "theirs added, mine kept"
        );
        let notes = s.notes.unwrap();
        assert!(notes.contains("my own note") && notes.contains("Check the Isaiah reading"));
        assert_eq!(s.title.as_deref(), Some("My title"));
    }

    #[test]
    fn a_label_colour_chosen_here_is_never_replaced_but_automatic_takes_the_folders() {
        let (_a, fa) = machine_a();
        let (mut b, fb) = machine_b();
        let sermons = b.create_label("sermons").unwrap();
        b.set_label_color(sermons, Some("#e01b24")).unwrap();
        let advent = b.create_label("Advent").unwrap();
        let _ = advent;
        pull(&fa, &fb);
        restore(&mut b, fb.path(), None);
        let by: std::collections::HashMap<String, Option<String>> = b
            .all_labels()
            .unwrap()
            .into_iter()
            .map(|l| (l.name, l.color_hex))
            .collect();
        assert_eq!(by["sermons"].as_deref(), Some("#e01b24"), "mine wins");
        assert_eq!(
            by.len(),
            2,
            "no duplicate made for a differently-cased name"
        );
    }

    #[test]
    fn an_existing_project_gets_missing_documents_after_its_own_and_keeps_its_root() {
        let (_a, fa) = machine_a();
        let (mut b, fb) = machine_b();
        let ch1 = doc(&mut b, &fb, "Thesis/ch1.typ");
        let main = doc(&mut b, &fb, "Thesis/main.typ");
        let p = b.create_project("thesis").unwrap();
        b.add_doc_to_project(p, main).unwrap();
        b.set_project_root(p, Some(main)).unwrap();
        let _ = ch1;
        pull(&fa, &fb);
        restore(&mut b, fb.path(), None);
        let snap = b.export_snapshot().unwrap();
        assert_eq!(snap.projects.len(), 1, "same name, any case: one project");
        assert_eq!(
            snap.projects[0].documents,
            vec![
                fb.path().join("Thesis/main.typ"),
                fb.path().join("Thesis/ch1.typ")
            ]
        );
        assert_eq!(
            snap.projects[0].root,
            Some(fb.path().join("Thesis/main.typ"))
        );
    }

    // ── two machines ─────────────────────────────────────────────────────

    #[test]
    fn a_pin_changed_on_another_machine_follows_once_the_two_have_agreed() {
        let (mut a, fa) = machine_a();
        let (mut b, fb) = machine_b();
        pull(&fa, &fb);
        restore(&mut b, fb.path(), None);
        export(&b, fb.path(), true, true);
        assert!(state(&b, &fb, "sermon.typ").pinned);

        // A unpins and exports; B pulls and has changed nothing there.
        let id = state(&a, &fa, "sermon.typ").id;
        a.set_pinned(id, false).unwrap();
        export(&a, fa.path(), true, true);
        pull(&fa, &fb);
        restore(&mut b, fb.path(), None);
        assert!(
            !state(&b, &fb, "sermon.typ").pinned,
            "their unpin is followed"
        );
    }

    #[test]
    fn a_title_renamed_on_both_machines_keeps_what_was_chosen_here() {
        let (mut a, fa) = machine_a();
        let (mut b, fb) = machine_b();
        pull(&fa, &fb);
        restore(&mut b, fb.path(), None);
        export(&b, fb.path(), true, true);

        let ida = state(&a, &fa, "sermon.typ").id;
        a.set_title(ida, "Renamed on A").unwrap();
        export(&a, fa.path(), true, true);
        let idb = state(&b, &fb, "sermon.typ").id;
        b.set_title(idb, "Renamed on B").unwrap();
        pull(&fa, &fb);
        restore(&mut b, fb.path(), None);
        assert_eq!(
            state(&b, &fb, "sermon.typ").title.as_deref(),
            Some("Renamed on B"),
            "a title changed here is never overwritten"
        );
    }

    #[test]
    fn two_machines_adding_different_things_converge_and_notes_stop_growing() {
        let (mut a, fa) = machine_a();
        let (mut b, fb) = machine_b();
        pull(&fa, &fb);
        restore(&mut b, fb.path(), None);
        export(&b, fb.path(), true, true);

        // Each machine adds its own label and writes its own note.
        let (ida, idb) = (
            state(&a, &fa, "sermon.typ").id,
            state(&b, &fb, "sermon.typ").id,
        );
        let x = a.create_label("FromA").unwrap();
        a.add_labels(&[ida], &[x]).unwrap();
        a.set_notes(ida, Some("Check the Isaiah reading\nA's addition"))
            .unwrap();
        let y = b.create_label("FromB").unwrap();
        b.add_labels(&[idb], &[y]).unwrap();
        b.set_notes(idb, Some("Check the Isaiah reading\nB's addition"))
            .unwrap();
        export(&a, fa.path(), true, true);
        export(&b, fb.path(), true, true);

        let mut lens = Vec::new();
        for _ in 0..4 {
            pull(&fa, &fb);
            restore(&mut b, fb.path(), None);
            export(&b, fb.path(), true, true);
            pull(&fb, &fa);
            restore(&mut a, fa.path(), None);
            export(&a, fa.path(), true, true);
            lens.push(state(&a, &fa, "sermon.typ").notes.unwrap().len());
        }
        for (m, f) in [(&a, &fa), (&b, &fb)] {
            let s = state(m, f, "sermon.typ");
            assert!(
                s.labels.contains(&"FromA".to_string()) && s.labels.contains(&"FromB".to_string())
            );
            let n = s.notes.unwrap();
            assert!(n.contains("A's addition") && n.contains("B's addition"));
        }
        assert_eq!(
            lens[1], lens[3],
            "the combined note settles instead of growing: {lens:?}"
        );
        assert_eq!(
            snapshot_files(&fa),
            snapshot_files(&fb),
            "and the two folders end up identical"
        );
    }

    // ── files that can't be used ─────────────────────────────────────────

    #[test]
    fn a_file_whose_document_isnt_here_yet_is_kept_for_later() {
        let (_a, fa) = machine_a();
        let (mut b, fb) = machine();
        doc(&mut b, &fb, "sermon.typ"); // the others haven't arrived
        pull(&fa, &fb);
        let r = restore(&mut b, fb.path(), None);
        assert!(r.waiting_for_document >= 1);
        assert_eq!(r.unreadable, 0);
        assert_eq!(
            state(&b, &fb, "sermon.typ").labels,
            vec!["Advent", "Sermons"]
        );

        // The project's documents arrive; the next pass completes it.
        doc(&mut b, &fb, "Thesis/main.typ");
        doc(&mut b, &fb, "Thesis/ch1.typ");
        let r2 = restore(&mut b, fb.path(), None);
        assert_eq!(r2.waiting_for_document, 0, "{r2:?}");
        assert_eq!(b.export_snapshot().unwrap().projects[0].documents.len(), 2);
        assert!(state(&b, &fb, "Thesis/ch1.typ").archived);
    }

    #[test]
    fn a_file_that_cannot_be_read_is_skipped_untouched_and_the_rest_still_merge() {
        let (a, fa) = machine_a();
        let (mut b, fb) = machine_b();
        pull(&fa, &fb);
        let bad = lib_dir(&fb)
            .join(a.machine_id().unwrap())
            .join("docs/Thesis/ch1.typ.toml");
        let conflicted =
            "<<<<<<< HEAD\narchived = true\n=======\narchived = false\n>>>>>>> other\n";
        std::fs::write(&bad, conflicted).unwrap();
        let r = restore(&mut b, fb.path(), None);
        assert_eq!(r.unreadable, 1);
        assert_eq!(std::fs::read_to_string(&bad).unwrap(), conflicted);
        assert!(!state(&b, &fb, "Thesis/ch1.typ").archived);
        assert_eq!(
            state(&b, &fb, "sermon.typ").labels,
            vec!["Advent", "Sermons"]
        );
        // …and the exporter keeps its hands off it too.
        export(&b, fb.path(), true, true);
        assert_eq!(std::fs::read_to_string(&bad).unwrap(), conflicted);
    }

    #[test]
    fn a_project_file_cannot_reach_outside_the_folder() {
        let (mut b, fb) = machine_b();
        let outside = TempDir::new().unwrap();
        let victim = outside.path().join("victim.typ");
        std::fs::write(&victim, "x").unwrap();
        let victim_id = b.upsert_document(&victim).unwrap();
        std::fs::create_dir_all(lib_dir(&fb).join("projects")).unwrap();
        let rel = format!(
            "../{}/victim.typ",
            outside.path().file_name().unwrap().to_string_lossy()
        );
        std::fs::write(
            lib_dir(&fb).join("projects/evil.toml"),
            format!(
                "name = \"Evil\"\ndocuments = [\"{rel}\", \"/etc/passwd\"]\nroot = \"{rel}\"\n"
            ),
        )
        .unwrap();
        restore(&mut b, fb.path(), None);
        let projects = b.export_snapshot().unwrap().projects;
        assert!(
            projects
                .iter()
                .all(|p| p.documents.is_empty() && p.root.is_none()),
            "{projects:?}"
        );
        let _ = victim_id;
    }

    #[test]
    fn an_empty_or_missing_export_folder_does_nothing() {
        let (mut b, fb) = machine_b();
        assert_eq!(restore(&mut b, fb.path(), None), RestoreReport::default());
        std::fs::create_dir_all(lib_dir(&fb)).unwrap();
        assert!(!restore(&mut b, fb.path(), None).changed_anything());
    }

    #[test]
    fn nothing_is_ever_removed_by_a_restore() {
        let (_a, fa) = machine_a();
        let (mut b, fb) = machine_b();
        let id = doc(&mut b, &fb, "sermon.typ");
        let extra = b.create_label("OnlyHere").unwrap();
        b.add_labels(&[id], &[extra]).unwrap();
        let p = b.create_project("OnlyHere").unwrap();
        b.add_doc_to_project(p, id).unwrap();
        let before = b.export_snapshot().unwrap();
        pull(&fa, &fb);
        restore(&mut b, fb.path(), None);
        let after = b.export_snapshot().unwrap();
        for l in &before.labels {
            assert!(
                after.labels.iter().any(|x| x.name == l.name),
                "label {} lost",
                l.name
            );
        }
        for p in &before.projects {
            let now = after
                .projects
                .iter()
                .find(|x| x.name == p.name)
                .expect("project lost");
            assert!(p.documents.iter().all(|d| now.documents.contains(d)));
        }
        assert_eq!(after.docs.len(), before.docs.len());
    }

    // ── the layouts and the ways a machine can be lost ───────────────────

    #[test]
    fn a_library_that_was_lost_gets_everything_back_from_its_own_old_folder() {
        // Same folder, same documents, but the library database is gone, so
        // this machine has a new name and its old folder looks like another's.
        let (_old, folder) = machine_a();
        let mut fresh = Library::open_in_memory();
        for n in ["Thesis/main.typ", "Thesis/ch1.typ", "sermon.typ"] {
            fresh.upsert_document(&folder.path().join(n)).unwrap();
        }
        let r = restore(&mut fresh, folder.path(), None);
        assert!(r.changed_anything());
        let s = state(&fresh, &folder, "sermon.typ");
        assert_eq!(s.labels, vec!["Advent", "Sermons"]);
        assert!(s.pinned);
        assert_eq!(s.notes.as_deref(), Some("Check the Isaiah reading"));
        assert_eq!(fresh.export_snapshot().unwrap().projects.len(), 1);
    }

    #[test]
    fn files_left_by_the_earlier_flat_layout_are_still_brought_in() {
        // The same data, but sitting directly under .zerkalo/library/ as the
        // first version of the export wrote it.
        let (a, fa) = machine_a();
        let (mut b, fb) = machine_b();
        let flat = lib_dir(&fb);
        let src = lib_dir(&fa).join(a.machine_id().unwrap());
        fn copy(from: &Path, to: &Path) {
            std::fs::create_dir_all(to).unwrap();
            for e in std::fs::read_dir(from).unwrap().flatten() {
                if e.path().is_dir() {
                    copy(&e.path(), &to.join(e.file_name()));
                } else {
                    std::fs::copy(e.path(), to.join(e.file_name())).unwrap();
                }
            }
        }
        copy(&src, &flat);
        let r = restore(&mut b, fb.path(), None);
        assert!(r.changed_anything());
        assert_eq!(
            state(&b, &fb, "sermon.typ").labels,
            vec!["Advent", "Sermons"]
        );
        // And the library's own export doesn't disturb them.
        let before = snapshot_files(&fb);
        export(&b, fb.path(), true, true);
        for (name, text) in &before {
            assert_eq!(
                snapshot_files(&fb).get(name),
                Some(text),
                "{name} was touched"
            );
        }
    }

    #[test]
    fn a_machine_never_merges_its_own_folder_back_into_itself() {
        let (mut a, fa) = machine_a();
        let id = state(&a, &fa, "sermon.typ").id;
        // The user removes a label; a stale copy of the old state sits in this
        // machine's own folder until the export catches up.
        let l = a
            .all_labels()
            .unwrap()
            .into_iter()
            .find(|l| l.name == "Advent")
            .unwrap();
        a.remove_labels(&[id], &[l.id]).unwrap();
        let r = restore(&mut a, fa.path(), None);
        assert!(!r.changed_anything());
        assert_eq!(
            state(&a, &fa, "sermon.typ").labels,
            vec!["Sermons"],
            "not resurrected"
        );
    }

    #[test]
    fn nothing_is_changed_if_the_safety_copy_cannot_be_saved() {
        let (_a, fa) = machine_a();
        let (mut b, fb) = machine_b();
        pull(&fa, &fb);
        let blocker = TempDir::new().unwrap();
        // A *file* where the backup folder should be.
        let not_a_dir = blocker.path().join("backups");
        std::fs::write(&not_a_dir, "x").unwrap();
        let before = b.export_snapshot().unwrap().docs;
        let r = restore(&mut b, fb.path(), Some(&not_a_dir));
        assert!(r.backup_failed);
        assert!(!r.changed_anything());
        assert_eq!(
            b.export_snapshot().unwrap().docs,
            before,
            "no change was made"
        );
        assert!(b.all_labels().unwrap().is_empty());
        assert!(b.export_snapshot().unwrap().projects.is_empty());
    }

    #[test]
    fn a_project_known_to_two_other_machines_is_made_once() {
        let (_a, fa) = machine_a();
        let (_a2, fa2) = machine_a();
        let (mut b, fb) = machine_b();
        // Two other machines, both with a project called Thesis.
        pull(&fa, &fb);
        pull(&fa2, &fb);
        restore(&mut b, fb.path(), None);
        let projects = b.export_snapshot().unwrap().projects;
        assert_eq!(projects.len(), 1, "{projects:?}");
        assert_eq!(projects[0].documents.len(), 2);
    }

    #[test]
    fn a_document_note_from_two_other_machines_keeps_both() {
        let (mut a1, f1) = machine_a();
        let (mut a2, f2) = machine_a();
        let i1 = state(&a1, &f1, "sermon.typ").id;
        let i2 = state(&a2, &f2, "sermon.typ").id;
        a1.set_notes(i1, Some("Check the Isaiah reading\nfrom one"))
            .unwrap();
        a2.set_notes(i2, Some("Check the Isaiah reading\nfrom two"))
            .unwrap();
        export(&a1, f1.path(), true, true);
        export(&a2, f2.path(), true, true);
        let (mut b, fb) = machine_b();
        pull(&f1, &fb);
        pull(&f2, &fb);
        restore(&mut b, fb.path(), None);
        let notes = state(&b, &fb, "sermon.typ").notes.unwrap();
        assert!(
            notes.contains("from one") && notes.contains("from two"),
            "{notes}"
        );
    }

    #[test]
    fn removing_a_label_on_one_machine_removes_it_on_the_other_and_it_stays_removed() {
        let (mut a, fa) = machine_a();
        let (mut b, fb) = machine_b();
        pull(&fa, &fb);
        restore(&mut b, fb.path(), None);
        export(&b, fb.path(), true, true);
        assert!(state(&b, &fb, "sermon.typ")
            .labels
            .contains(&"Advent".to_string()));

        // A takes Advent off the sermon.
        let (ida, advent) = (
            state(&a, &fa, "sermon.typ").id,
            a.all_labels()
                .unwrap()
                .into_iter()
                .find(|l| l.name == "Advent")
                .unwrap()
                .id,
        );
        a.remove_labels(&[ida], &[advent]).unwrap();
        export(&a, fa.path(), true, true);
        pull(&fa, &fb);
        restore(&mut b, fb.path(), None);
        export(&b, fb.path(), true, true);
        assert_eq!(
            state(&b, &fb, "sermon.typ").labels,
            vec!["Sermons"],
            "it followed"
        );

        // And B's next export doesn't bring it back to A.
        pull(&fb, &fa);
        restore(&mut a, fa.path(), None);
        assert_eq!(
            state(&a, &fa, "sermon.typ").labels,
            vec!["Sermons"],
            "and stayed gone"
        );
        for _ in 0..2 {
            pull(&fa, &fb);
            restore(&mut b, fb.path(), None);
            pull(&fb, &fa);
            restore(&mut a, fa.path(), None);
        }
        assert_eq!(state(&a, &fa, "sermon.typ").labels, vec!["Sermons"]);
        assert_eq!(state(&b, &fb, "sermon.typ").labels, vec!["Sermons"]);
    }

    #[test]
    fn the_tree_stamp_notices_a_file_arriving() {
        let (_a, fa) = machine_a();
        let (_b, fb) = machine_b();
        let empty = tree_stamp(fb.path());
        assert_eq!(empty.0, 0);
        pull(&fa, &fb);
        assert!(tree_stamp(fb.path()).0 > 0);
        assert_ne!(tree_stamp(fb.path()), empty);
    }
}

/// Real git, two clones and a shared remote: the whole point of keeping this
/// data in the folder is that it travels through the folder's git, so what
/// happens to the *writing* when two machines change the same label matters
/// more than anything else here.
#[cfg(test)]
mod sync_tests {
    use super::*;
    use crate::git_sync;
    use crate::library_export::export;
    use std::process::Command;
    use tempfile::TempDir;

    fn git(dir: &Path, args: &[&str]) -> String {
        let out = Command::new("git")
            .current_dir(dir)
            .args(args)
            .output()
            .expect("git runs");
        assert!(
            out.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).into_owned()
    }

    fn identity(dir: &Path) {
        git(dir, &["config", "user.name", "T"]);
        git(dir, &["config", "user.email", "t@example.com"]);
        git(dir, &["config", "commit.gpgsign", "false"]);
    }

    /// A shared remote and two clones, each with the same two documents.
    fn two_machines() -> (TempDir, TempDir, TempDir, Library, Library) {
        let remote = TempDir::new().unwrap();
        git(remote.path(), &["init", "--bare", "-b", "main"]);
        let a = TempDir::new().unwrap();
        git(a.path(), &["init", "-b", "main"]);
        identity(a.path());
        git(
            a.path(),
            &["remote", "add", "origin", remote.path().to_str().unwrap()],
        );
        std::fs::write(a.path().join("sermon.typ"), "= Sermon\nFirst draft.\n").unwrap();
        std::fs::write(a.path().join("essay.typ"), "= Essay\nFirst draft.\n").unwrap();
        git(a.path(), &["add", "."]);
        git(a.path(), &["commit", "-m", "start"]);
        git(a.path(), &["push", "-u", "origin", "main"]);
        let b = TempDir::new().unwrap();
        git(b.path(), &["clone", remote.path().to_str().unwrap(), "."]);
        identity(b.path());
        let (mut la, mut lb) = (Library::open_in_memory(), Library::open_in_memory());
        for (lib, dir) in [(&mut la, &a), (&mut lb, &b)] {
            for n in ["sermon.typ", "essay.typ"] {
                lib.upsert_document(&dir.path().join(n)).unwrap();
            }
        }
        (remote, a, b, la, lb)
    }

    fn id_state(lib: &Library, dir: &TempDir, name: &str) -> crate::library::DocState {
        let path = dir.path().join(name);
        lib.export_snapshot()
            .unwrap()
            .docs
            .into_iter()
            .find(|d| d.path == path)
            .unwrap()
    }

    fn id_of(lib: &Library, dir: &TempDir, name: &str) -> i64 {
        let path = dir.path().join(name);
        lib.export_snapshot()
            .unwrap()
            .docs
            .into_iter()
            .find(|d| d.path == path)
            .unwrap()
            .id
    }

    #[test]
    fn two_machines_labelling_the_same_document_still_back_up_the_writing() {
        let (_remote, a, b, mut la, mut lb) = two_machines();

        // Each labels and pins the same document its own way, exports, and
        // does what Save & Back Up does.
        let (ia, ib) = (id_of(&la, &a, "sermon.typ"), id_of(&lb, &b, "sermon.typ"));
        let x = la.create_label("FromLaptop").unwrap();
        la.add_labels(&[ia], &[x]).unwrap();
        la.set_pinned(ia, true).unwrap();
        la.set_notes(ia, Some("laptop note")).unwrap();
        let y = lb.create_label("FromDesktop").unwrap();
        lb.add_labels(&[ib], &[y]).unwrap();
        lb.set_notes(ib, Some("desktop note")).unwrap();
        export(&la, a.path(), true, true);
        export(&lb, b.path(), true, true);

        // And writing, in the same documents, on both.
        std::fs::write(a.path().join("essay.typ"), "= Essay\nLaptop paragraph.\n").unwrap();
        std::fs::write(
            b.path().join("sermon.typ"),
            "= Sermon\nDesktop paragraph.\n",
        )
        .unwrap();

        let first = git_sync::sync(a.path(), None);
        assert!(first.pushed && first.push_errors.is_empty(), "{first:?}");
        let second = git_sync::sync(b.path(), None);
        assert!(
            second.pushed && second.push_errors.is_empty(),
            "the second machine's backup of its writing must not be blocked: {second:?}"
        );

        // Nothing written was lost, on either side.
        assert_eq!(
            std::fs::read_to_string(b.path().join("essay.typ")).unwrap(),
            "= Essay\nLaptop paragraph.\n"
        );
        assert_eq!(
            std::fs::read_to_string(b.path().join("sermon.typ")).unwrap(),
            "= Sermon\nDesktop paragraph.\n"
        );
        let third = git_sync::sync(a.path(), None);
        assert!(third.push_errors.is_empty(), "{third:?}");
        assert_eq!(
            std::fs::read_to_string(a.path().join("sermon.typ")).unwrap(),
            "= Sermon\nDesktop paragraph.\n"
        );
        assert_eq!(
            git(b.path(), &["status", "--porcelain"]).trim(),
            "",
            "nothing left half-merged"
        );
    }

    #[test]
    fn labels_travel_through_git_to_the_other_machine_and_nothing_written_is_touched() {
        let (_remote, a, b, mut la, mut lb) = two_machines();
        let ia = id_of(&la, &a, "sermon.typ");
        let advent = la.create_label("Advent").unwrap();
        la.add_labels(&[ia], &[advent]).unwrap();
        la.set_notes(ia, Some("remember the Magnificat")).unwrap();
        la.set_pinned(ia, true).unwrap();
        let p = la.create_project("Thesis").unwrap();
        la.add_doc_to_project(p, ia).unwrap();
        export(&la, a.path(), true, true);
        let writing_a = std::fs::read_to_string(a.path().join("essay.typ")).unwrap();

        assert!(git_sync::sync(a.path(), None).pushed);
        let pulled = git_sync::sync(b.path(), None);
        assert!(pulled.pushed && pulled.push_errors.is_empty(), "{pulled:?}");

        // The other machine merges what arrived…
        let r = restore(&mut lb, b.path(), None);
        assert!(r.changed_anything(), "{r:?}");
        let s = id_state(&lb, &b, "sermon.typ");
        assert_eq!(s.labels, vec!["Advent"]);
        assert!(s.pinned);
        assert_eq!(s.notes.as_deref(), Some("remember the Magnificat"));
        assert_eq!(lb.export_snapshot().unwrap().projects[0].name, "Thesis");
        // …and writes it into its own folder, which then syncs back cleanly.
        export(&lb, b.path(), true, true);
        let back = git_sync::sync(b.path(), None);
        assert!(back.pushed && back.push_errors.is_empty(), "{back:?}");
        let again = git_sync::sync(a.path(), None);
        assert!(again.push_errors.is_empty(), "{again:?}");

        // No document was changed by any of it.
        assert_eq!(
            std::fs::read_to_string(a.path().join("essay.typ")).unwrap(),
            writing_a
        );
        assert_eq!(
            std::fs::read_to_string(b.path().join("essay.typ")).unwrap(),
            "= Essay\nFirst draft.\n"
        );
        assert_eq!(git(a.path(), &["status", "--porcelain"]).trim(), "");
        assert_eq!(git(b.path(), &["status", "--porcelain"]).trim(), "");
        // Both clones have both machines' folders and no conflict markers anywhere.
        let log = git(a.path(), &["log", "--oneline"]);
        assert!(!log.is_empty());
        for root in [&a, &b] {
            let tree = git(root.path(), &["ls-files", ".zerkalo/library"]);
            let dirs: std::collections::HashSet<&str> =
                tree.lines().filter_map(|l| l.split('/').nth(2)).collect();
            assert_eq!(dirs.len(), 2, "{tree}");
        }
    }
}
