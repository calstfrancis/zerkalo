//! Which bibliography a document cites from — decided in exactly one place.
//!
//! Precedence, most specific first:
//!   1. the document's own `#bibliography("…")` line, when that file exists
//!      (it is what the document compiles against, so it is what the editor
//!      should offer keys from);
//!   2. the project's setting (`.zerkalo/config.toml`);
//!   3. the global setting (Settings → Folders);
//!   4. a bibliography found in the project folder.
//!
//! Resolving never touches a file: it only reads and decides.

use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origin {
    Document,
    Project,
    Settings,
    Found,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Resolved {
    /// A `.bib`/`.yaml` file, or a Kartoteka vault folder.
    pub path: PathBuf,
    pub origin: Origin,
}

/// A `library.yml` inside a Kartoteka vault means the vault itself, so
/// entries keep loading live from `entries/` rather than from the export.
fn vault_for(path: &Path) -> Option<PathBuf> {
    if path.file_name().and_then(|n| n.to_str()) == Some("library.yml") {
        let dir = path.parent()?;
        if crate::bibliography::is_vault_dir(dir) {
            return Some(dir.to_path_buf());
        }
    }
    None
}

fn from_document(doc: &Path, text: Option<&str>) -> Option<PathBuf> {
    let owned;
    let text = match text {
        Some(t) => t,
        None => {
            owned = std::fs::read_to_string(doc).ok()?;
            &owned
        }
    };
    let named = Path::new(crate::styles::find_bibliography_path(text)?);
    let full = if named.is_absolute() {
        named.to_path_buf()
    } else {
        doc.parent().unwrap_or(Path::new(".")).join(named)
    };
    if !full.is_file() {
        return None;
    }
    Some(vault_for(&full).unwrap_or(full))
}

/// The one bibliography in the project folder, preferring `.bib` and, among
/// several, the first by name — so the choice never depends on directory
/// order.
fn found_in(project_root: &Path) -> Option<PathBuf> {
    let mut bibs = Vec::new();
    let mut yamls = Vec::new();
    for entry in std::fs::read_dir(project_root).ok()?.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        match path
            .extension()
            .and_then(|x| x.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref()
        {
            Some("bib") => bibs.push(path),
            Some("yaml") | Some("yml") => yamls.push(path),
            _ => {}
        }
    }
    bibs.sort();
    yamls.sort();
    bibs.into_iter().next().or_else(|| yamls.into_iter().next())
}

pub fn resolve(
    doc: Option<(&Path, Option<&str>)>,
    project: Option<&Path>,
    global: Option<&Path>,
    project_root: &Path,
) -> Option<Resolved> {
    if let Some(path) = doc.and_then(|(d, text)| from_document(d, text)) {
        return Some(Resolved {
            path,
            origin: Origin::Document,
        });
    }
    if let Some(path) = project {
        return Some(Resolved {
            path: path.to_path_buf(),
            origin: Origin::Project,
        });
    }
    if let Some(path) = global {
        return Some(Resolved {
            path: path.to_path_buf(),
            origin: Origin::Settings,
        });
    }
    found_in(project_root).map(|path| Resolved {
        path,
        origin: Origin::Found,
    })
}

/// How to write `source` in a `#bibliography(...)` line of `doc`: relative to
/// the document when the file sits beside it or below it (so the folder can
/// move between computers), otherwise the full path.
pub fn path_for_document(source: &Path, doc: &Path) -> PathBuf {
    let target = crate::bibliography::bib_target_path(source);
    match doc.parent().and_then(|dir| target.strip_prefix(dir).ok()) {
        Some(rel) if !rel.as_os_str().is_empty() => rel.to_path_buf(),
        _ => target,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(dir: &Path, name: &str, body: &str) -> PathBuf {
        let p = dir.join(name);
        std::fs::write(&p, body).unwrap();
        p
    }

    #[test]
    fn the_document_wins_over_every_setting() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "mine.bib", "@book{a,}");
        let doc = write(dir.path(), "p.typ", "#bibliography(\"mine.bib\")");
        let proj = dir.path().join("proj.bib");
        let glob = dir.path().join("glob.bib");
        let r = resolve(Some((&doc, None)), Some(&proj), Some(&glob), dir.path()).unwrap();
        assert_eq!(r.origin, Origin::Document);
        assert_eq!(r.path, dir.path().join("mine.bib"));
    }

    #[test]
    fn unsaved_text_is_what_counts() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "new.bib", "");
        let doc = dir.path().join("p.typ");
        std::fs::write(&doc, "no bibliography yet").unwrap();
        let r = resolve(
            Some((&doc, Some("#bibliography(\"new.bib\")"))),
            None,
            None,
            dir.path(),
        )
        .unwrap();
        assert_eq!(r.path, dir.path().join("new.bib"));
    }

    #[test]
    fn a_missing_document_file_falls_back_to_settings() {
        let dir = tempfile::tempdir().unwrap();
        let doc = write(dir.path(), "p.typ", "#bibliography(\"gone.bib\")");
        let proj = PathBuf::from("/somewhere/proj.bib");
        let r = resolve(Some((&doc, None)), Some(&proj), None, dir.path()).unwrap();
        assert_eq!((r.origin, r.path), (Origin::Project, proj));
    }

    #[test]
    fn project_beats_global_beats_found() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "x.bib", "");
        let g = PathBuf::from("/g.bib");
        let p = PathBuf::from("/p.bib");
        assert_eq!(
            resolve(None, Some(&p), Some(&g), dir.path()).unwrap().path,
            p
        );
        assert_eq!(resolve(None, None, Some(&g), dir.path()).unwrap().path, g);
        assert_eq!(
            resolve(None, None, None, dir.path()).unwrap().origin,
            Origin::Found
        );
    }

    #[test]
    fn finding_is_deterministic_and_prefers_bib() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "docker-compose.yml", "services: {}");
        write(dir.path(), "b.bib", "");
        write(dir.path(), "a.bib", "");
        let r = resolve(None, None, None, dir.path()).unwrap();
        assert_eq!(r.path, dir.path().join("a.bib"));
    }

    #[test]
    fn nothing_anywhere_is_none() {
        let dir = tempfile::tempdir().unwrap();
        assert!(resolve(None, None, None, dir.path()).is_none());
    }

    #[test]
    fn a_vaults_library_yml_means_the_vault() {
        let dir = tempfile::tempdir().unwrap();
        let vault = dir.path().join("vault");
        std::fs::create_dir_all(vault.join("entries")).unwrap();
        write(&vault, "library.yml", "");
        let doc = write(
            dir.path(),
            "p.typ",
            &format!("#bibliography(\"{}\")", vault.join("library.yml").display()),
        );
        let r = resolve(Some((&doc, None)), None, None, dir.path()).unwrap();
        assert_eq!(r.path, vault);
    }

    #[test]
    fn inside_the_project_the_document_gets_a_relative_path() {
        let dir = tempfile::tempdir().unwrap();
        let doc = dir.path().join("essay.typ");
        let bib = dir.path().join("lib").join("refs.bib");
        assert_eq!(path_for_document(&bib, &doc), PathBuf::from("lib/refs.bib"));
        let elsewhere = PathBuf::from("/other/place/refs.bib");
        assert_eq!(path_for_document(&elsewhere, &doc), elsewhere);
    }
}
