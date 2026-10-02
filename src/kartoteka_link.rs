//! Telling Kartoteka which documents a project contains.
//!
//! Kartoteka already knows how to show "Used in: …" on every source, given a
//! small file in the vault's `projects/` folder listing the Typst documents a
//! project comprises. With the person's permission, Zerkalo writes exactly
//! one such file per computer and project, and nothing else in the vault.
//!
//! The file name carries this computer's name, so two computers sharing a
//! vault through git never write the same file.

use std::path::{Path, PathBuf};

const MARKER: &str = "# zerkalo-project: written by Zerkalo; safe to delete.";

fn slug(text: &str) -> String {
    let s: String = text
        .chars()
        .map(|c| {
            if c.is_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    let s = s
        .split('-')
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    if s.is_empty() {
        "project".into()
    } else {
        s
    }
}

/// The file Zerkalo would write for `project_root` on the computer called `host`.
pub fn file_for(vault: &Path, project_root: &Path, host: &str) -> PathBuf {
    let name = project_root
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("project");
    vault
        .join(fond_bib::library::PROJECTS_DIR)
        .join(format!("{}-{}.yml", slug(name), slug(host)))
}

/// Writes (or refreshes) the project file. Returns the file if it changed.
/// Never touches a file it didn't write.
pub fn sync(vault: &Path, project_root: &Path, host: &str) -> std::io::Result<Option<PathBuf>> {
    let path = file_for(vault, project_root, host);
    if path.exists() {
        let first = std::fs::read_to_string(&path)?;
        if !first.starts_with(MARKER) {
            return Ok(None);
        }
    }
    let name = project_root
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("project")
        .to_string();
    let project = fond_bib::project::Project {
        name,
        description: None,
        documents: crate::project::collect_typ_files(project_root),
    };
    let yaml = project
        .to_text()
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    let text = format!("{MARKER}\n{yaml}");
    if std::fs::read_to_string(&path).ok().as_deref() == Some(text.as_str()) {
        return Ok(None);
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    crate::bibliography::write_atomic(&path, &text)?;
    Ok(Some(path))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_one_marked_file_listing_the_documents() {
        let vault = tempfile::tempdir().unwrap();
        let proj = tempfile::tempdir().unwrap();
        std::fs::write(proj.path().join("essay.typ"), "x").unwrap();
        let written = sync(vault.path(), proj.path(), "Cal's T14")
            .unwrap()
            .unwrap();
        assert!(written.starts_with(vault.path().join("projects")));
        assert!(written.to_string_lossy().ends_with("-cal-s-t14.yml"));
        let text = std::fs::read_to_string(&written).unwrap();
        assert!(text.starts_with(MARKER));
        assert!(text.contains("essay.typ"));
        // Nothing else was created in the vault.
        let others: Vec<_> = std::fs::read_dir(vault.path()).unwrap().flatten().collect();
        assert_eq!(others.len(), 1);
        // Unchanged → no rewrite.
        assert!(sync(vault.path(), proj.path(), "Cal's T14")
            .unwrap()
            .is_none());
    }

    #[test]
    fn a_file_kartoteka_or_the_person_wrote_is_left_alone() {
        let vault = tempfile::tempdir().unwrap();
        let proj = tempfile::tempdir().unwrap();
        let path = file_for(vault.path(), proj.path(), "h");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "name: mine\ndocuments: []\n").unwrap();
        assert!(sync(vault.path(), proj.path(), "h").unwrap().is_none());
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "name: mine\ndocuments: []\n"
        );
    }

    #[test]
    fn the_file_parses_as_a_kartoteka_project() {
        let vault = tempfile::tempdir().unwrap();
        let proj = tempfile::tempdir().unwrap();
        std::fs::write(proj.path().join("a.typ"), "x").unwrap();
        let p = sync(vault.path(), proj.path(), "h").unwrap().unwrap();
        let parsed =
            fond_bib::project::Project::parse(&std::fs::read_to_string(&p).unwrap(), &p).unwrap();
        assert_eq!(parsed.documents.len(), 1);
    }
}
