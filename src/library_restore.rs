//! Bringing labels, projects, pins and notes back from the Zerkalo folder
//! (`library_export` wrote them there) into the library — on a new machine,
//! after moving the folder, or when another machine's changes arrive in it.
//!
//! The rule is **merge, never replace, never remove**:
//!
//! - labels and project members are only ever *added*;
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
//! has been merged, its content is recorded as agreed, so the export that
//! follows may update it with the combined result.

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
    /// Files that couldn't be read as what they should be; left untouched.
    pub unreadable: usize,
    /// Files naming a document the library doesn't have (yet). They are tried
    /// again next time rather than being given up on.
    pub waiting_for_document: usize,
    /// Whether a copy of the library was saved before changing it.
    pub backed_up: bool,
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
    let have: HashSet<String> = ours.labels.iter().map(|l| l.to_lowercase()).collect();
    let mut seen = HashSet::new();
    let add_labels = theirs
        .labels
        .iter()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .filter(|l| !have.contains(&l.to_lowercase()) && seen.insert(l.to_lowercase()))
        .map(String::from)
        .collect();

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
}

impl Backup<'_> {
    fn before_change(&mut self, lib: &Library) {
        if self.done {
            return;
        }
        self.done = true;
        let Some(dir) = self.dir else { return };
        std::fs::create_dir_all(dir).ok();
        let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
        let dest = dir.join(format!("library.before-restore-{stamp}.sqlite"));
        if lib.backup_to(&dest).is_err() {
            tracing::warn!("Couldn't save a copy of the library before restoring");
            return;
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
    let root = folder.join(DIR);
    let mut files = Vec::new();
    collect(&root, &root, &mut files);
    if files.is_empty() {
        return report;
    }
    // Labels first, so a document can use a colour that came with them.
    files.sort_by_key(|(name, _)| match name.as_str() {
        "labels.toml" => 0,
        n if n.starts_with("docs/") => 1,
        _ => 2,
    });

    let records = lib.export_records().unwrap_or_default();
    let mut backup = Backup {
        dir: backup_dir,
        done: false,
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
    let projects: HashMap<String, ProjectState> = snapshot
        .projects
        .iter()
        .map(|p| (p.name.to_lowercase(), p.clone()))
        .collect();
    let mut labels: HashMap<String, Option<String>> = snapshot
        .labels
        .iter()
        .map(|l| (l.name.to_lowercase(), l.color_hex.clone()))
        .collect();

    for (name, path) in files {
        let Ok(text) = std::fs::read_to_string(&path) else {
            report.unreadable += 1;
            continue;
        };
        let recorded = records.get(&name);
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
                        backup.before_change(lib);
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
                            backup.before_change(lib);
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
                backup.before_change(lib);
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
                if let Some(p) = plan.pinned {
                    lib.set_pinned(doc.id, p).ok();
                }
                if let Some(a) = plan.archived {
                    lib.set_archived(doc.id, a).ok();
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
                backup.before_change(lib);
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
                }
            }
        } else {
            // README.txt and anything else: not ours to interpret.
            continue;
        }

        if complete {
            // Agreed: from here the export may update this file with the merge.
            lib.set_export_record(&name, &text).ok();
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
    fn labels_are_only_ever_added_and_matched_ignoring_case() {
        let mut mine = ours();
        mine.labels = vec!["Advent".into()];
        let theirs = DocIn {
            labels: vec!["advent".into(), "Lent".into(), "lent".into(), " ".into()],
            ..Default::default()
        };
        assert_eq!(plan_doc(&mine, &theirs, None).add_labels, vec!["Lent"]);
        // Their file having *fewer* labels asks for nothing to be removed.
        let fewer = DocIn::default();
        assert!(plan_doc(&mine, &fewer, None).is_empty());
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
        assert!(export(&lib, folder.path(), true).errors.is_empty());
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
        let (_a, fa) = machine_a();
        let (mut b, fb) = machine_b();
        pull(&fa, &fb);
        restore(&mut b, fb.path(), None);
        let report = export(&b, fb.path(), true);
        assert_eq!(report.left_alone, 0, "nothing is stuck waiting");
        assert_eq!(snapshot_files(&fb), snapshot_files(&fa));
    }

    #[test]
    fn restoring_again_changes_nothing_and_takes_no_backup() {
        let (_a, fa) = machine_a();
        let (mut b, fb) = machine_b();
        let backups = TempDir::new().unwrap();
        pull(&fa, &fb);
        let first = restore(&mut b, fb.path(), Some(backups.path()));
        assert!(first.changed_anything() && first.backed_up);
        export(&b, fb.path(), true);
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
        export(&b, fb.path(), true);
        assert!(state(&b, &fb, "sermon.typ").pinned);

        // A unpins and exports; B pulls and has changed nothing there.
        let id = state(&a, &fa, "sermon.typ").id;
        a.set_pinned(id, false).unwrap();
        export(&a, fa.path(), true);
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
        export(&b, fb.path(), true);

        let ida = state(&a, &fa, "sermon.typ").id;
        a.set_title(ida, "Renamed on A").unwrap();
        export(&a, fa.path(), true);
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
        export(&b, fb.path(), true);

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
        export(&a, fa.path(), true);
        export(&b, fb.path(), true);

        let mut lens = Vec::new();
        for _ in 0..4 {
            pull(&fa, &fb);
            restore(&mut b, fb.path(), None);
            export(&b, fb.path(), true);
            pull(&fb, &fa);
            restore(&mut a, fa.path(), None);
            export(&a, fa.path(), true);
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
        let (_a, fa) = machine_a();
        let (mut b, fb) = machine_b();
        pull(&fa, &fb);
        let bad = lib_dir(&fb).join("docs/Thesis/ch1.typ.toml");
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
        export(&b, fb.path(), true);
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
