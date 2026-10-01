//! A copy of the library's own data — labels, projects, pins, notes — kept in
//! the Zerkalo folder, so it is backed up with the folder (and so travels with
//! it) instead of living only in a database outside it.
//!
//! This is **export only**: nothing here reads the files back, so nothing the
//! library holds depends on them. And it is built never to harm anything that
//! is already there:
//!
//! - it only writes under `<folder>/.zerkalo/library/`, never to a document;
//! - it creates files, or updates one only when the file still holds exactly
//!   what this library last wrote to it. A file that differs (changed on
//!   another machine and pulled in, edited by hand, or already there when this
//!   library first looked) is left alone, whatever this library thinks;
//! - writes are atomic (a temporary file, then a rename), so a crash or a full
//!   disk can't leave half a file;
//! - it never deletes.
//!
//! One small file per document, one per project, one for the labels' colours,
//! and a README saying what all this is.
//!
//! **Each machine writes only inside its own folder**, `.zerkalo/library/<id>/`.
//! That is what keeps this from ever getting in the way of git: two machines
//! never edit the same file, so a `pull --rebase` of the folder can't conflict
//! over this data, and the backup of your writing is never held up by it.
//! Other machines' folders are only ever read (see `library_restore`).
//!
//! Notes are left out unless asked for (`include_notes`): they are private
//! writing, and this folder is usually pushed somewhere. Everything else here
//! is a label, a pin or a name.

use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};

use serde::Serialize;

use crate::library::{DocState, Label, Library, ProjectState};

/// Where the files go, under the Zerkalo folder.
pub const DIR: &str = ".zerkalo/library";

#[derive(Debug, Default, PartialEq)]
pub struct ExportReport {
    pub written: usize,
    pub unchanged: usize,
    /// Files that exist but aren't what this library last wrote, so were not
    /// touched.
    pub left_alone: usize,
    /// Documents living outside the folder, which have no place in it.
    pub outside_folder: usize,
    pub errors: Vec<String>,
}

const README: &str = "\
This folder is one machine's copy of what you've done in the Library: labels,
projects and pins (and notes, if you've chosen to include them — they are left
out unless you turn that on in the Library's View menu). It lives here so it is backed up with the rest of
your Zerkalo folder, and so another machine can bring it in.

  docs/        one file per document, named after it
  projects/    one file per project, with its documents in order
  labels.toml  your labels and the colours you chose for them

Each machine that uses this folder has a folder of its own beside this one, and
writes only to that — so they never get in each other's way, in git or
anywhere else. Zerkalo reads the others' folders and merges them in; it never
changes them. It never deletes anything here and never overwrites a file it did
not write itself. Change labels and projects in the Library, not in these files.
";

#[derive(Serialize)]
struct DocFile<'a> {
    labels: &'a [String],
    pinned: bool,
    archived: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    title: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    notes: Option<&'a str>,
}

#[derive(Serialize)]
struct ProjectFile<'a> {
    name: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    root: Option<String>,
    documents: Vec<String>,
}

#[derive(Serialize)]
struct LabelEntry<'a> {
    name: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    color: Option<&'a str>,
}

#[derive(Serialize)]
struct LabelsFile<'a> {
    label: Vec<LabelEntry<'a>>,
}

/// `path` relative to `folder`, as `/`-separated plain components — or `None`
/// if it isn't inside, or has anything odd in it (`..`, a root) that could
/// point a file somewhere it shouldn't.
fn relative(path: &Path, folder: &Path) -> Option<String> {
    let rel = path.strip_prefix(folder).ok()?;
    let mut parts = Vec::new();
    for c in rel.components() {
        match c {
            Component::Normal(p) => parts.push(p.to_str()?.to_string()),
            _ => return None,
        }
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("/"))
    }
}

fn render_doc(rel: &str, d: &DocState) -> String {
    let body = toml::to_string(&DocFile {
        labels: &d.labels,
        pinned: d.pinned,
        archived: d.archived,
        title: d.title.as_deref(),
        notes: d.notes.as_deref(),
    })
    .unwrap_or_default();
    format!(
        "# Zerkalo library data for {rel}. Zerkalo writes this file; see ../README.txt.\n{body}"
    )
}

fn render_project(p: &ProjectState, folder: &Path) -> String {
    let rels = |paths: &[PathBuf]| -> Vec<String> {
        paths.iter().filter_map(|p| relative(p, folder)).collect()
    };
    let body = toml::to_string(&ProjectFile {
        name: &p.name,
        root: p.root.as_deref().and_then(|r| relative(r, folder)),
        documents: rels(&p.documents),
    })
    .unwrap_or_default();
    format!("# A Zerkalo project. Zerkalo writes this file; see ../README.txt.\n{body}")
}

fn render_labels(labels: &[Label]) -> String {
    let mut sorted: Vec<&Label> = labels.iter().collect();
    sorted.sort_by_key(|l| l.name.to_lowercase());
    let body = toml::to_string(&LabelsFile {
        label: sorted
            .iter()
            .map(|l| LabelEntry {
                name: &l.name,
                color: l.color_hex.as_deref(),
            })
            .collect(),
    })
    .unwrap_or_default();
    format!("# Zerkalo labels. Zerkalo writes this file; see README.txt.\n{body}")
}

/// A file-name-safe version of a project's name.
fn slug(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect();
    let s = s.trim_matches('-').to_string();
    if s.is_empty() {
        "project".into()
    } else {
        s
    }
}

/// A crash between writing a file and renaming it leaves a `.tmp-zerkalo`
/// file behind, which a sync would then commit. These are ours, in our own
/// folder; clearing them up is all that is ever deleted here.
fn remove_stale_temp_files(root: &Path) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            remove_stale_temp_files(&p);
        } else if p.extension().is_some_and(|x| x == "tmp-zerkalo") {
            std::fs::remove_file(&p).ok();
        }
    }
}

/// Writes `content` to `path` through a temporary file, so the file is always
/// either the old version or the new one.
fn write_atomically(path: &Path, content: &str) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("tmp-zerkalo");
    let result = std::fs::write(&tmp, content).and_then(|_| std::fs::rename(&tmp, path));
    if result.is_err() {
        std::fs::remove_file(&tmp).ok();
    }
    result
}

/// Brings the export folder up to date with the library.
///
/// `verify_disk` reads each file that is expected to be unchanged to check it
/// really is; otherwise a file whose content hasn't changed since it was last
/// written is not even looked at, which keeps frequent calls cheap.
pub fn export(
    lib: &Library,
    folder: &Path,
    verify_disk: bool,
    include_notes: bool,
) -> ExportReport {
    let mut report = ExportReport::default();
    let mut snapshot = match lib.export_snapshot() {
        Ok(s) => s,
        Err(e) => {
            report
                .errors
                .push(format!("couldn't read the library: {e}"));
            return report;
        }
    };
    let id = match lib.machine_id() {
        Ok(id) => id,
        Err(e) => {
            report
                .errors
                .push(format!("couldn't name this machine: {e}"));
            return report;
        }
    };
    if !include_notes {
        // Not written, not even into a file that is being written anyway; a
        // file that already has them is rewritten without.
        for d in &mut snapshot.docs {
            d.notes = None;
        }
    }
    let mut records = lib.export_records().unwrap_or_default();
    let root = folder.join(DIR).join(&id);

    if verify_disk {
        remove_stale_temp_files(&root);
    }

    let mut files: Vec<(String, String)> = vec![("README.txt".into(), README.into())];

    for d in &snapshot.docs {
        let Some(rel) = relative(&d.path, folder) else {
            report.outside_folder += 1;
            continue;
        };
        let name = format!("docs/{rel}.toml");
        let nothing_to_say = d.labels.is_empty()
            && !d.pinned
            && !d.archived
            && d.title.is_none()
            && d.notes.is_none();
        // Don't litter the folder with a file saying nothing — but a file that
        // already exists is brought up to date when its document empties out.
        if nothing_to_say && !records.contains_key(&format!("{id}/{name}")) {
            continue;
        }
        files.push((name, render_doc(&rel, d)));
    }

    let mut used: HashMap<String, usize> = HashMap::new();
    for p in &snapshot.projects {
        let base = slug(&p.name);
        let n = used.entry(base.clone()).or_insert(0);
        *n += 1;
        let name = if *n == 1 {
            format!("projects/{base}.toml")
        } else {
            format!("projects/{base}-{n}.toml")
        };
        files.push((name, render_project(p, folder)));
    }
    if !snapshot.labels.is_empty() || records.contains_key(&format!("{id}/labels.toml")) {
        files.push(("labels.toml".into(), render_labels(&snapshot.labels)));
    }

    for (name, content) in files {
        apply(
            lib,
            &root,
            &id,
            &name,
            &content,
            &mut records,
            verify_disk,
            &mut report,
        );
    }
    report
}

#[allow(clippy::too_many_arguments)]
fn apply(
    lib: &Library,
    root: &Path,
    id: &str,
    name: &str,
    content: &str,
    records: &mut HashMap<String, String>,
    verify_disk: bool,
    report: &mut ExportReport,
) {
    let path = root.join(name);
    // Belt and braces: nothing may land outside the export folder.
    if !path.starts_with(root) || name.split('/').any(|c| c == ".." || c.is_empty()) {
        report.errors.push(format!("refused to write {name}"));
        return;
    }
    // Recorded under the machine's id too, so these records can't be mistaken
    // for those of files left by an earlier layout or by another machine.
    let key = format!("{id}/{name}");
    let recorded = records.get(&key).cloned();

    // Unchanged since last time, and not asked to double-check the disk.
    if !verify_disk && recorded.as_deref() == Some(content) {
        report.unchanged += 1;
        return;
    }

    let existing = match std::fs::read_to_string(&path) {
        Ok(text) => Some(text),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        // Present but unreadable (not UTF-8, permissions): not ours to touch.
        Err(_) => {
            report.left_alone += 1;
            return;
        }
    };

    let may_write = match (&existing, &recorded) {
        // Nothing there: create it.
        (None, _) => true,
        // It holds exactly what this library last wrote: ours to update.
        (Some(on_disk), Some(last)) => on_disk == last,
        // It's there and this library has no record of writing it. If it
        // already says what we'd say, adopt it; otherwise leave it be.
        (Some(on_disk), None) => {
            if on_disk == content {
                lib.set_export_record(&key, content).ok();
                records.insert(key.clone(), content.to_string());
                report.unchanged += 1;
                return;
            }
            false
        }
    };
    if !may_write {
        report.left_alone += 1;
        return;
    }
    if existing.as_deref() == Some(content) {
        lib.set_export_record(&key, content).ok();
        records.insert(key.clone(), content.to_string());
        report.unchanged += 1;
        return;
    }
    match write_atomically(&path, content) {
        Ok(()) => {
            // Only once it is safely on disk does the library remember it.
            lib.set_export_record(&key, content).ok();
            records.insert(key.clone(), content.to_string());
            report.written += 1;
        }
        Err(e) => report.errors.push(format!("{name}: {e}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    /// A library whose documents live in a temp folder, with a few real ones.
    fn setup() -> (Library, TempDir) {
        let folder = TempDir::new().unwrap();
        let lib = Library::open_in_memory();
        (lib, folder)
    }

    fn add(lib: &mut Library, folder: &TempDir, name: &str) -> i64 {
        let path = folder.path().join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "= Heading\nBody.\n").unwrap();
        lib.upsert_document(&path).unwrap()
    }

    /// A file in this library's own export folder.
    fn file(lib: &Library, folder: &TempDir, rel: &str) -> PathBuf {
        folder
            .path()
            .join(DIR)
            .join(lib.machine_id().unwrap())
            .join(rel)
    }

    fn read(lib: &Library, folder: &TempDir, rel: &str) -> String {
        std::fs::read_to_string(file(lib, folder, rel)).unwrap_or_else(|_| panic!("no {rel}"))
    }

    #[test]
    fn a_document_with_labels_a_pin_and_notes_gets_its_own_file() {
        let (mut lib, folder) = setup();
        let id = add(&mut lib, &folder, "essay.typ");
        let advent = lib.create_label("Advent").unwrap();
        let sermons = lib.create_label("Sermons").unwrap();
        lib.add_labels(&[id], &[advent, sermons]).unwrap();
        lib.set_pinned(id, true).unwrap();
        lib.set_notes(id, Some("Check the Isaiah reading")).unwrap();

        let r = export(&lib, folder.path(), true, true);
        assert!(r.errors.is_empty(), "{:?}", r.errors);

        let text = read(&lib, &folder, "docs/essay.typ.toml");
        let parsed: toml::Value = toml::from_str(&text).expect("valid TOML");
        assert_eq!(
            parsed["labels"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_str().unwrap())
                .collect::<Vec<_>>(),
            vec!["Advent", "Sermons"]
        );
        assert_eq!(parsed["pinned"].as_bool(), Some(true));
        assert_eq!(parsed["archived"].as_bool(), Some(false));
        assert_eq!(parsed["notes"].as_str(), Some("Check the Isaiah reading"));
        assert!(
            parsed.get("title").is_none(),
            "an untouched title isn't saved"
        );
    }

    #[test]
    fn a_renamed_title_is_saved_but_a_derived_one_is_not() {
        let (mut lib, folder) = setup();
        let a = add(&mut lib, &folder, "a.typ");
        let b = add(&mut lib, &folder, "b.typ");
        lib.set_title(a, "My Name For It").unwrap();
        lib.set_pinned(b, true).unwrap();
        export(&lib, folder.path(), true, true);
        assert!(read(&lib, &folder, "docs/a.typ.toml").contains("title = \"My Name For It\""));
        assert!(!read(&lib, &folder, "docs/b.typ.toml").contains("title"));
    }

    #[test]
    fn documents_with_nothing_to_say_get_no_file() {
        let (mut lib, folder) = setup();
        add(&mut lib, &folder, "plain.typ");
        export(&lib, folder.path(), true, true);
        assert!(!file(&lib, &folder, "docs/plain.typ.toml").exists());
        assert!(file(&lib, &folder, "README.txt").exists());
    }

    #[test]
    fn documents_in_subfolders_keep_their_place() {
        let (mut lib, folder) = setup();
        let id = add(&mut lib, &folder, "Thesis/ch1.typ");
        lib.set_pinned(id, true).unwrap();
        export(&lib, folder.path(), true, true);
        assert!(file(&lib, &folder, "docs/Thesis/ch1.typ.toml").exists());
    }

    #[test]
    fn a_document_outside_the_folder_is_counted_and_skipped() {
        let (mut lib, folder) = setup();
        let elsewhere = TempDir::new().unwrap();
        let path = elsewhere.path().join("away.typ");
        std::fs::write(&path, "x").unwrap();
        let id = lib.upsert_document(&path).unwrap();
        lib.set_pinned(id, true).unwrap();
        let r = export(&lib, folder.path(), true, true);
        assert_eq!(r.outside_folder, 1);
        assert!(!file(&lib, &folder, "docs").exists());
    }

    #[test]
    fn projects_are_saved_with_their_documents_in_order_and_their_root() {
        let (mut lib, folder) = setup();
        let a = add(&mut lib, &folder, "Thesis/main.typ");
        let b = add(&mut lib, &folder, "Thesis/ch1.typ");
        let c = add(&mut lib, &folder, "Thesis/ch2.typ");
        let p = lib.create_project("My Thesis").unwrap();
        for d in [c, a, b] {
            lib.add_doc_to_project(p, d).unwrap();
        }
        lib.set_project_root(p, Some(a)).unwrap();
        export(&lib, folder.path(), true, true);
        let parsed: toml::Value =
            toml::from_str(&read(&lib, &folder, "projects/My-Thesis.toml")).unwrap();
        assert_eq!(parsed["name"].as_str(), Some("My Thesis"));
        assert_eq!(parsed["root"].as_str(), Some("Thesis/main.typ"));
        let docs: Vec<&str> = parsed["documents"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert_eq!(
            docs,
            vec!["Thesis/ch2.typ", "Thesis/main.typ", "Thesis/ch1.typ"]
        );
    }

    #[test]
    fn two_projects_with_one_name_get_two_files() {
        let (mut lib, folder) = setup();
        lib.create_project("Same").unwrap();
        lib.create_project("Same").unwrap();
        export(&lib, folder.path(), true, true);
        assert!(file(&lib, &folder, "projects/Same.toml").exists());
        assert!(file(&lib, &folder, "projects/Same-2.toml").exists());
    }

    #[test]
    fn labels_are_saved_with_only_the_colours_that_were_chosen() {
        let (mut lib, folder) = setup();
        let a = lib.create_label("Zeta").unwrap();
        lib.create_label("alpha").unwrap();
        lib.set_label_color(a, Some("#e01b24")).unwrap();
        export(&lib, folder.path(), true, true);
        let parsed: toml::Value = toml::from_str(&read(&lib, &folder, "labels.toml")).unwrap();
        let l = parsed["label"].as_array().unwrap();
        assert_eq!(l[0]["name"].as_str(), Some("alpha"));
        assert!(l[0].get("color").is_none());
        assert_eq!(l[1]["color"].as_str(), Some("#e01b24"));
    }

    #[test]
    fn exporting_again_with_nothing_changed_writes_nothing() {
        let (mut lib, folder) = setup();
        let id = add(&mut lib, &folder, "a.typ");
        lib.set_pinned(id, true).unwrap();
        let first = export(&lib, folder.path(), true, true);
        assert!(first.written > 0);
        for verify in [false, true] {
            let again = export(&lib, folder.path(), verify, true);
            assert_eq!(again.written, 0, "verify={verify}");
            assert_eq!(again.left_alone, 0);
        }
    }

    #[test]
    fn a_change_updates_the_file_and_emptying_a_document_updates_it_too() {
        let (mut lib, folder) = setup();
        let id = add(&mut lib, &folder, "a.typ");
        let l = lib.create_label("Advent").unwrap();
        lib.add_labels(&[id], &[l]).unwrap();
        export(&lib, folder.path(), true, true);
        assert!(read(&lib, &folder, "docs/a.typ.toml").contains("Advent"));

        lib.remove_labels(&[id], &[l]).unwrap();
        let r = export(&lib, folder.path(), false, true);
        assert_eq!(r.written, 1, "only the document's file changed");
        let text = read(&lib, &folder, "docs/a.typ.toml");
        assert!(!text.contains("Advent"), "the stale label is gone: {text}");
        assert!(
            file(&lib, &folder, "docs/a.typ.toml").exists(),
            "never deleted"
        );
    }

    // ── the promises ─────────────────────────────────────────────────────

    #[test]
    fn a_file_that_was_already_there_is_never_overwritten() {
        // The library has no record of writing what's in its folder (it was
        // lost and rebuilt, or restored from an old copy). Those files must
        // survive untouched, however the library's own state differs.
        let (mut lib, folder) = setup();
        let id = add(&mut lib, &folder, "a.typ");
        lib.set_pinned(id, true).unwrap();
        let theirs = "labels = [\"From Another Machine\"]\npinned = false\narchived = false\n";
        std::fs::create_dir_all(file(&lib, &folder, "docs")).unwrap();
        std::fs::write(file(&lib, &folder, "docs/a.typ.toml"), theirs).unwrap();

        for _ in 0..3 {
            let r = export(&lib, folder.path(), true, true);
            assert_eq!(r.left_alone, 1);
            assert_eq!(read(&lib, &folder, "docs/a.typ.toml"), theirs);
        }
    }

    #[test]
    fn a_file_changed_after_this_library_wrote_it_is_left_alone_from_then_on() {
        // What `git pull` from another machine looks like.
        let (mut lib, folder) = setup();
        let id = add(&mut lib, &folder, "a.typ");
        lib.set_pinned(id, true).unwrap();
        export(&lib, folder.path(), true, true);
        let pulled = "labels = [\"Pulled\"]\npinned = true\narchived = false\n";
        std::fs::write(file(&lib, &folder, "docs/a.typ.toml"), pulled).unwrap();

        lib.set_notes(id, Some("a local change")).unwrap();
        let r = export(&lib, folder.path(), true, true);
        assert_eq!(r.left_alone, 1);
        assert_eq!(read(&lib, &folder, "docs/a.typ.toml"), pulled);
    }

    #[test]
    fn a_file_with_merge_conflict_markers_is_left_exactly_as_it_is() {
        let (mut lib, folder) = setup();
        let id = add(&mut lib, &folder, "a.typ");
        lib.set_pinned(id, true).unwrap();
        export(&lib, folder.path(), true, true);
        let conflicted = "<<<<<<< HEAD\npinned = true\n=======\npinned = false\n>>>>>>> other\n";
        std::fs::write(file(&lib, &folder, "docs/a.typ.toml"), conflicted).unwrap();
        lib.set_pinned(id, false).unwrap();
        export(&lib, folder.path(), true, true);
        assert_eq!(read(&lib, &folder, "docs/a.typ.toml"), conflicted);
    }

    #[test]
    fn a_file_that_already_says_the_same_thing_is_adopted_not_rewritten() {
        let (mut lib, folder) = setup();
        let id = add(&mut lib, &folder, "a.typ");
        lib.set_pinned(id, true).unwrap();
        export(&lib, folder.path(), true, true);
        // The library forgets what it wrote (restored from an old copy, say).
        lib.forget_exports();
        let r = export(&lib, folder.path(), true, true);
        assert_eq!(r.left_alone, 0);
        assert_eq!(r.written, 0);
        // …and from then on it may update that file.
        lib.set_pinned(id, false).unwrap();
        export(&lib, folder.path(), true, true);
        assert!(read(&lib, &folder, "docs/a.typ.toml").contains("pinned = false"));
    }

    #[test]
    fn a_deleted_export_file_is_made_again() {
        let (mut lib, folder) = setup();
        let id = add(&mut lib, &folder, "a.typ");
        lib.set_pinned(id, true).unwrap();
        export(&lib, folder.path(), true, true);
        std::fs::remove_file(file(&lib, &folder, "docs/a.typ.toml")).unwrap();
        export(&lib, folder.path(), true, true);
        assert!(file(&lib, &folder, "docs/a.typ.toml").exists());
    }

    #[test]
    fn the_export_never_touches_a_document_or_anything_outside_its_folder() {
        let (mut lib, folder) = setup();
        let id = add(&mut lib, &folder, "a.typ");
        lib.set_pinned(id, true).unwrap();
        let before = std::fs::read_to_string(folder.path().join("a.typ")).unwrap();
        std::fs::write(folder.path().join("keep.txt"), "mine").unwrap();
        export(&lib, folder.path(), true, true);
        assert_eq!(
            std::fs::read_to_string(folder.path().join("a.typ")).unwrap(),
            before
        );
        assert_eq!(
            std::fs::read_to_string(folder.path().join("keep.txt")).unwrap(),
            "mine"
        );
        // Everything new is under the export folder.
        let mut stray = Vec::new();
        for e in std::fs::read_dir(folder.path()).unwrap().flatten() {
            let n = e.file_name().to_string_lossy().into_owned();
            if !["a.typ", "keep.txt", ".zerkalo"].contains(&n.as_str()) {
                stray.push(n);
            }
        }
        assert!(stray.is_empty(), "{stray:?}");
    }

    #[test]
    fn odd_paths_cannot_escape_the_folder() {
        let folder = Path::new("/work");
        assert_eq!(
            relative(Path::new("/work/a.typ"), folder).as_deref(),
            Some("a.typ")
        );
        assert_eq!(
            relative(Path::new("/work/x/../../etc/passwd"), folder),
            None
        );
        assert_eq!(relative(Path::new("/elsewhere/a.typ"), folder), None);
        assert_eq!(relative(Path::new("/work"), folder), None);
    }

    #[test]
    fn no_temporary_file_is_left_behind() {
        let (mut lib, folder) = setup();
        let id = add(&mut lib, &folder, "a.typ");
        lib.set_pinned(id, true).unwrap();
        export(&lib, folder.path(), true, true);
        fn walk(dir: &Path, out: &mut Vec<String>) {
            for e in std::fs::read_dir(dir).unwrap().flatten() {
                let p = e.path();
                if p.is_dir() {
                    walk(&p, out);
                } else {
                    out.push(p.to_string_lossy().into_owned());
                }
            }
        }
        let mut all = Vec::new();
        walk(&folder.path().join(DIR), &mut all);
        assert!(all.iter().all(|f| !f.contains("tmp-zerkalo")), "{all:?}");
    }

    #[test]
    fn an_export_folder_that_cannot_be_made_changes_nothing_and_records_nothing() {
        let (mut lib, folder) = setup();
        let id = add(&mut lib, &folder, "a.typ");
        lib.set_pinned(id, true).unwrap();
        // A *file* where the export folder's parent should be a directory.
        std::fs::write(folder.path().join(".zerkalo"), "not a directory").unwrap();
        let r = export(&lib, folder.path(), true, true);
        assert_eq!(r.written, 0);
        assert!(lib.export_records().unwrap().is_empty());
        assert_eq!(
            std::fs::read_to_string(folder.path().join(".zerkalo")).unwrap(),
            "not a directory"
        );
    }

    #[test]
    fn notes_with_awkward_characters_survive_as_valid_toml() {
        let (mut lib, folder) = setup();
        let id = add(&mut lib, &folder, "a.typ");
        let notes = "line one\nline \"two\" with a \\ backslash and 'quotes'\n\tтест — Žižek ✝";
        lib.set_notes(id, Some(notes)).unwrap();
        export(&lib, folder.path(), true, true);
        let parsed: toml::Value = toml::from_str(&read(&lib, &folder, "docs/a.typ.toml")).unwrap();
        assert_eq!(parsed["notes"].as_str(), Some(notes.trim()));
    }

    #[test]
    fn documents_in_the_trash_are_not_exported() {
        let (mut lib, folder) = setup();
        let id = add(&mut lib, &folder, "a.typ");
        lib.set_pinned(id, true).unwrap();
        lib.move_to_trash(id).unwrap();
        export(&lib, folder.path(), true, true);
        assert!(!file(&lib, &folder, "docs/a.typ.toml").exists());
    }

    #[test]
    fn the_change_stamp_moves_when_the_library_does() {
        let (mut lib, folder) = setup();
        let id = add(&mut lib, &folder, "a.typ");
        let before = lib.change_stamp();
        lib.set_pinned(id, true).unwrap();
        assert_ne!(lib.change_stamp(), before);
    }

    // ── one folder per machine ───────────────────────────────────────────

    #[test]
    fn a_machine_writes_only_inside_its_own_folder() {
        let (mut lib, folder) = setup();
        let id = add(&mut lib, &folder, "a.typ");
        lib.set_pinned(id, true).unwrap();
        export(&lib, folder.path(), true, true);
        let entries: Vec<String> = std::fs::read_dir(folder.path().join(DIR))
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(entries, vec![lib.machine_id().unwrap()]);
    }

    #[test]
    fn two_machines_never_write_the_same_file() {
        let (mut a, folder) = setup();
        let mut b = Library::open_in_memory();
        for lib in [&mut a, &mut b] {
            let id = lib.upsert_document(&{
                let p = folder.path().join("shared.typ");
                std::fs::write(&p, "x").unwrap();
                p
            });
            lib.set_pinned(id.unwrap(), true).unwrap();
        }
        export(&a, folder.path(), true, true);
        export(&b, folder.path(), true, true);
        assert_ne!(a.machine_id().unwrap(), b.machine_id().unwrap());
        assert!(file(&a, &folder, "docs/shared.typ.toml").exists());
        assert!(file(&b, &folder, "docs/shared.typ.toml").exists());
        assert_ne!(
            file(&a, &folder, "docs/shared.typ.toml"),
            file(&b, &folder, "docs/shared.typ.toml")
        );
    }

    #[test]
    fn a_machines_name_is_made_once_and_kept() {
        let (lib, _folder) = setup();
        let first = lib.machine_id().unwrap();
        assert!(first.starts_with("m-") && first.len() > 6);
        assert_eq!(lib.machine_id().unwrap(), first);
        assert_ne!(Library::open_in_memory().machine_id().unwrap(), first);
    }

    #[test]
    fn leftover_temporary_files_in_our_own_folder_are_cleared_and_nothing_else_is() {
        let (mut lib, folder) = setup();
        let id = add(&mut lib, &folder, "a.typ");
        lib.set_pinned(id, true).unwrap();
        export(&lib, folder.path(), true, true);
        let mine = folder.path().join(DIR).join(lib.machine_id().unwrap());
        let theirs = folder.path().join(DIR).join("m-someone-else");
        std::fs::create_dir_all(theirs.join("docs")).unwrap();
        std::fs::write(mine.join("docs/crashed.tmp-zerkalo"), "half").unwrap();
        std::fs::write(mine.join("docs/note.txt"), "a file somebody put here").unwrap();
        std::fs::write(theirs.join("docs/other.tmp-zerkalo"), "not ours").unwrap();
        export(&lib, folder.path(), true, true);
        assert!(!mine.join("docs/crashed.tmp-zerkalo").exists());
        assert!(
            mine.join("docs/note.txt").exists(),
            "only our own temporaries go"
        );
        assert!(
            theirs.join("docs/other.tmp-zerkalo").exists(),
            "another machine's folder is never touched"
        );
    }

    /// Not run by default: how long the export and a restore take on a big
    /// library, since both run on the main thread. `cargo test big_library --
    /// --ignored --nocapture`.
    #[test]
    #[ignore]
    fn big_library_timings() {
        use std::time::Instant;
        let (mut lib, folder) = setup();
        let n = 4000;
        let t = Instant::now();
        let labels: Vec<i64> = (0..40)
            .map(|i| lib.create_label(&format!("label {i}")).unwrap())
            .collect();
        for i in 0..n {
            let id = add(&mut lib, &folder, &format!("d{}/doc{i}.typ", i % 40));
            lib.add_labels(&[id], &[labels[i % 40], labels[(i * 7) % 40]])
                .unwrap();
            if i % 3 == 0 {
                lib.set_notes(id, Some("a note about this one")).unwrap();
            }
        }
        println!("built {n} documents in {:?}", t.elapsed());
        let t = Instant::now();
        let r = export(&lib, folder.path(), true, true);
        println!(
            "first export (verify): {:?}, {} files written",
            t.elapsed(),
            r.written
        );
        let id = lib.export_snapshot().unwrap().docs[5].id;
        lib.set_pinned(id, true).unwrap();
        let t = Instant::now();
        let r = export(&lib, folder.path(), false, true);
        println!(
            "after one change (no disk check): {:?}, {} written",
            t.elapsed(),
            r.written
        );
        let t = Instant::now();
        export(&lib, folder.path(), true, true);
        println!("full verify pass, nothing changed: {:?}", t.elapsed());
        // A second machine restoring all of it.
        let mut b = Library::open_in_memory();
        for i in 0..n {
            b.upsert_document(&folder.path().join(format!("d{}/doc{i}.typ", i % 40)))
                .unwrap();
        }
        let t = Instant::now();
        let r = crate::library_restore::restore(&mut b, folder.path(), None);
        println!(
            "restore of everything onto a new machine: {:?} ({} documents)",
            t.elapsed(),
            r.documents
        );
        let t = Instant::now();
        let r = crate::library_restore::restore(&mut b, folder.path(), None);
        println!(
            "a second restore pass (nothing new): {:?} ({} documents)",
            t.elapsed(),
            r.documents
        );
    }

    // ── notes are opt-in ─────────────────────────────────────────────────

    #[test]
    fn notes_are_left_out_unless_asked_for_and_everything_else_still_goes() {
        let (mut lib, folder) = setup();
        let id = add(&mut lib, &folder, "a.typ");
        let l = lib.create_label("Advent").unwrap();
        lib.add_labels(&[id], &[l]).unwrap();
        lib.set_pinned(id, true).unwrap();
        lib.set_notes(id, Some("a private thought")).unwrap();
        export(&lib, folder.path(), true, false);
        let text = read(&lib, &folder, "docs/a.typ.toml");
        assert!(
            !text.contains("private") && !text.contains("notes"),
            "{text}"
        );
        assert!(text.contains("Advent") && text.contains("pinned = true"));
        // Nowhere else in the folder either.
        let mut all = String::new();
        fn walk(d: &Path, out: &mut String) {
            for e in std::fs::read_dir(d).unwrap().flatten() {
                let p = e.path();
                if p.is_dir() {
                    walk(&p, out);
                } else {
                    out.push_str(&std::fs::read_to_string(&p).unwrap());
                }
            }
        }
        walk(&folder.path().join(DIR), &mut all);
        assert!(!all.contains("a private thought"));
    }

    #[test]
    fn a_document_with_only_a_note_gets_no_file_while_notes_are_left_out() {
        let (mut lib, folder) = setup();
        let id = add(&mut lib, &folder, "a.typ");
        lib.set_notes(id, Some("only a note")).unwrap();
        export(&lib, folder.path(), true, false);
        assert!(!file(&lib, &folder, "docs/a.typ.toml").exists());
        export(&lib, folder.path(), true, true);
        assert!(read(&lib, &folder, "docs/a.typ.toml").contains("only a note"));
    }

    #[test]
    fn turning_notes_on_adds_them_and_turning_them_off_takes_them_out_of_our_files() {
        let (mut lib, folder) = setup();
        let id = add(&mut lib, &folder, "a.typ");
        lib.set_pinned(id, true).unwrap();
        lib.set_notes(id, Some("a private thought")).unwrap();
        export(&lib, folder.path(), true, false);
        assert!(!read(&lib, &folder, "docs/a.typ.toml").contains("private"));
        let on = export(&lib, folder.path(), false, true);
        assert_eq!(on.written, 1);
        assert!(read(&lib, &folder, "docs/a.typ.toml").contains("a private thought"));
        let off = export(&lib, folder.path(), false, false);
        assert_eq!(off.written, 1);
        assert!(!read(&lib, &folder, "docs/a.typ.toml").contains("private"));
        // The library itself never lost the note.
        assert_eq!(
            lib.export_snapshot().unwrap().docs[0].notes.as_deref(),
            Some("a private thought")
        );
    }
}
