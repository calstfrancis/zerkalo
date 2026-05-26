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
