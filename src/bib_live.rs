//! What a bibliography holds right now.
//!
//! Two kinds of file can fall behind the records they stand for:
//!
//! - a Kartoteka vault's `library.yml`. Kartoteka rewrites it after imports
//!   and merges, but not after every field edit, so a date added in
//!   Kartoteka sat in `entries/` while Typst kept reading the old export.
//!   The vault's `entries/` are the truth, and this builds the same
//!   document `library.yml` would hold from them.
//! - a Zerkalo copy of a bibliography kept outside the project
//!   (`bib_mirror`). It is refreshed from its original on a timer, and only
//!   for the bibliography Settings names. When the original is reachable,
//!   this reads through to it.
//!
//! Everything that compiles, renders or exports from a bibliography reads it
//! through here, so the preview, Word citations and exported reference lists
//! agree with the Citations panel, which reads the vault live.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Changes whenever what [`read`] returns may have changed. Cheap: it only
/// stats files.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stamp {
    files: usize,
    newest: Option<SystemTime>,
}

impl Stamp {
    fn of(path: &Path) -> Option<Self> {
        let modified = std::fs::metadata(path).and_then(|m| m.modified()).ok()?;
        Some(Self {
            files: 1,
            newest: Some(modified),
        })
    }

    fn and(self, other: Self) -> Self {
        Self {
            files: self.files + other.files,
            newest: self.newest.max(other.newest),
        }
    }
}

/// The vault `path` stands for: the vault folder itself, or its `library.yml`.
fn vault_of(path: &Path) -> Option<PathBuf> {
    if crate::bibliography::is_vault_dir(path) {
        return Some(path.to_path_buf());
    }
    if path.file_name().and_then(|n| n.to_str()) != Some("library.yml") {
        return None;
    }
    path.parent()
        .filter(|dir| crate::bibliography::is_vault_dir(dir))
        .map(Path::to_path_buf)
}

/// The vault's entries as one Hayagriva document, in the same order and form
/// Kartoteka writes `library.yml` (`Library::regenerate_library_yml`). An
/// entry that doesn't parse is left out, as the Citations panel leaves it out.
/// A vault with no entries at all leaves `library.yml` to speak for itself.
fn vault_yaml(vault: &Path) -> Option<String> {
    let library = fond_bib::Library::open(vault).ok()?;
    let keys = library.keys_sorted().ok().filter(|k| !k.is_empty())?;
    let mut lib = hayagriva::Library::new();
    for key in keys {
        match library.load_entry(&key) {
            Ok(parsed) => lib.push(&parsed.entry),
            Err(e) => tracing::warn!("Leaving out vault entry {key}: {e}"),
        }
    }
    if lib.is_empty() {
        return Some(String::new());
    }
    hayagriva::io::to_yaml_str(&lib).ok()
}

/// What an original bibliography holds: a file's text, or a vault's entries.
/// Never follows a Zerkalo copy (see [`read`]).
pub fn read_original(path: &Path) -> std::io::Result<String> {
    if let Some(vault) = vault_of(path) {
        if let Some(text) = vault_yaml(&vault) {
            return Ok(text);
        }
    }
    std::fs::read_to_string(crate::bibliography::bib_target_path(path))
}

fn original_stamp(path: &Path) -> Option<Stamp> {
    let Some(vault) = vault_of(path) else {
        return Stamp::of(path);
    };
    let entries = vault.join("entries");
    // The folder's own time moves when an entry is added, removed or
    // replaced (Kartoteka writes by rename); the files' times when one is
    // edited in place.
    let mut stamp = Stamp::of(&entries)?;
    if let Some(s) = Stamp::of(&vault.join("library.yml")) {
        stamp = stamp.and(s);
    }
    for entry in std::fs::read_dir(&entries).ok()?.flatten() {
        let p = entry.path();
        if p.extension().and_then(|e| e.to_str()) == Some("yml") {
            if let Some(s) = Stamp::of(&p) {
                stamp = stamp.and(s);
            }
        }
    }
    Some(stamp)
}

/// The original a Zerkalo copy was made from, when it is on this computer.
fn reachable_origin(path: &Path) -> Option<PathBuf> {
    crate::bib_mirror::origin_of(path).filter(|o| o.exists())
}

/// The text a bibliography should be read as now. For a Zerkalo copy whose
/// original is on this computer, that is the copy as it is about to be
/// refreshed to; elsewhere the copy itself, which is what git carried there.
pub fn read(path: &Path) -> std::io::Result<String> {
    if let Some(origin) = reachable_origin(path) {
        if let Ok(body) = read_original(&origin) {
            return Ok(crate::bib_mirror::marked(path, &origin, &body));
        }
    }
    read_original(path)
}

/// [`read`], but only when it differs in kind from the file on disk: `None`
/// for an ordinary file, which the caller can read itself.
pub fn derived_text(path: &Path) -> Option<String> {
    (vault_of(path).is_some() || reachable_origin(path).is_some())
        .then(|| read(path).ok())
        .flatten()
}

/// Moves whenever [`read`]'s answer for `path` may have.
pub fn stamp(path: &Path) -> Option<Stamp> {
    let own = original_stamp(path);
    match reachable_origin(path).and_then(|o| original_stamp(&o)) {
        Some(origin) => Some(own.map_or(origin, |s| s.and(origin))),
        None => own,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vault_with(dir: &Path, entries: &[(&str, &str)]) {
        std::fs::create_dir_all(dir.join("entries")).unwrap();
        for (key, body) in entries {
            std::fs::write(dir.join("entries").join(format!("{key}.yml")), body).unwrap();
        }
    }

    const SMITH: &str = "smith2020:\n  type: book\n  title: A Book\n  author: Smith, Jane\n";
    const SMITH_DATED: &str =
        "smith2020:\n  type: book\n  title: A Book\n  author: Smith, Jane\n  date: 2020-05-01\n";

    #[test]
    fn a_vault_is_read_from_its_entries_not_a_stale_library_yml() {
        let v = tempfile::tempdir().unwrap();
        vault_with(v.path(), &[("smith2020", SMITH)]);
        // library.yml as Kartoteka last wrote it, before the date was added.
        std::fs::write(v.path().join("library.yml"), SMITH).unwrap();
        std::fs::write(v.path().join("entries/smith2020.yml"), SMITH_DATED).unwrap();

        for path in [v.path().to_path_buf(), v.path().join("library.yml")] {
            let text = read(&path).unwrap();
            let lib = hayagriva::io::from_yaml_str(&text).unwrap();
            let entry = lib.get("smith2020").unwrap();
            assert_eq!(
                entry.date().map(|d| d.year),
                Some(2020),
                "{}",
                path.display()
            );
            assert!(derived_text(&path).is_some());
        }
    }

    #[test]
    fn editing_an_entry_moves_the_stamp() {
        let v = tempfile::tempdir().unwrap();
        vault_with(v.path(), &[("smith2020", SMITH)]);
        let before = stamp(&v.path().join("library.yml")).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        std::fs::write(v.path().join("entries/smith2020.yml"), SMITH_DATED).unwrap();
        assert_ne!(stamp(&v.path().join("library.yml")).unwrap(), before);
    }

    #[test]
    fn a_copy_reads_through_to_its_original_while_it_is_reachable() {
        let proj = tempfile::tempdir().unwrap();
        let out = tempfile::tempdir().unwrap();
        let src = out.path().join("zotero.bib");
        std::fs::write(&src, "@book{a, title={Old}}\n").unwrap();
        let copy = crate::bib_mirror::mirror_path_for(proj.path(), &src);
        crate::bib_mirror::sync(&src, &copy).unwrap();
        let before = stamp(&copy).unwrap();

        std::thread::sleep(std::time::Duration::from_millis(20));
        std::fs::write(&src, "@book{a, title={New}, date={2021}}\n").unwrap();
        let text = read(&copy).unwrap();
        assert!(text.contains("title={New}"), "{text}");
        assert!(crate::bib_mirror::is_ours_text(&text));
        assert_ne!(stamp(&copy).unwrap(), before);

        // On a computer without the original, the copy is what there is.
        std::fs::remove_file(&src).unwrap();
        assert!(derived_text(&copy).is_none());
        assert!(read(&copy).unwrap().contains("title={Old}"));
    }

    #[test]
    fn an_ordinary_file_is_left_to_the_caller() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("refs.bib");
        std::fs::write(&p, "@book{a,}\n").unwrap();
        assert!(derived_text(&p).is_none());
        assert_eq!(read(&p).unwrap(), "@book{a,}\n");
        assert!(stamp(&p).is_some());
    }
}
