use std::path::{Path, PathBuf};

/// Recursively collect all `.typ` files under `root`, respecting `.gitignore`.
pub fn collect_typ_files(root: &Path) -> Vec<PathBuf> {
    let repo = git2::Repository::open(root).ok();
    let mut files = Vec::new();
    collect_recursive(root, root, &repo, &mut files);
    files.sort();
    files
}

fn collect_recursive(
    root: &Path,
    dir: &Path,
    repo: &Option<git2::Repository>,
    out: &mut Vec<PathBuf>,
) {
    let mut entries: Vec<_> = match std::fs::read_dir(dir) {
        Ok(rd) => rd.flatten().collect(),
        Err(_) => return,
    };
    entries.sort_by_key(|e| e.file_name());

    for entry in entries {
        let path = entry.path();
        if entry.file_name().to_string_lossy().starts_with('.') {
            continue;
        }
        if let Some(repo) = repo {
            if repo.is_path_ignored(&path).unwrap_or(false) {
                continue;
            }
        }
        if path.is_dir() {
            collect_recursive(root, &path, repo, out);
        } else if path.extension().map(|e| e == "typ").unwrap_or(false) {
            out.push(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collect_typ_files_finds_nested_typ_files_only() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("main.typ"), "content").unwrap();
        std::fs::write(dir.path().join("notes.md"), "not typst").unwrap();
        std::fs::create_dir(dir.path().join("chapters")).unwrap();
        std::fs::write(dir.path().join("chapters/ch1.typ"), "content").unwrap();

        let files = collect_typ_files(dir.path());
        let names: Vec<String> = files.iter()
            .map(|f| f.strip_prefix(dir.path()).unwrap().to_string_lossy().to_string())
            .collect();
        assert_eq!(names, vec!["chapters/ch1.typ", "main.typ"]);
    }

    #[test]
    fn collect_typ_files_skips_hidden_entries() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("main.typ"), "content").unwrap();
        std::fs::create_dir(dir.path().join(".hidden")).unwrap();
        std::fs::write(dir.path().join(".hidden/secret.typ"), "content").unwrap();

        let files = collect_typ_files(dir.path());
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].file_name().unwrap(), "main.typ");
    }

    #[test]
    fn collect_typ_files_empty_dir_returns_empty() {
        let dir = tempfile::tempdir().unwrap();
        assert!(collect_typ_files(dir.path()).is_empty());
    }
}
