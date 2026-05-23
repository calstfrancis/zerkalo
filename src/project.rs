use std::path::{Path, PathBuf};

use crate::error::Result;

const MAIN_TYP_TEMPLATE: &str = "\
#set document(title: \"My Document\", author: \"\")
#set page(paper: \"a4\", margin: (x: 2.5cm, y: 2.5cm))
#set text(size: 11pt)
#set par(justify: true)

= Introduction

Your document begins here.
";

const GITIGNORE: &str = "*.pdf\n*.png\n.zerkalo/cache/\n";

pub fn init_project(path: &Path) -> Result<()> {
    std::fs::create_dir_all(path)?;
    std::fs::create_dir_all(path.join(".zerkalo"))?;

    let main_typ = path.join("main.typ");
    if !main_typ.exists() {
        std::fs::write(&main_typ, MAIN_TYP_TEMPLATE)?;
    }

    let gitignore = path.join(".gitignore");
    if !gitignore.exists() {
        std::fs::write(&gitignore, GITIGNORE)?;
    }

    if git2::Repository::open(path).is_err() {
        git2::Repository::init(path)?;
    }

    Ok(())
}

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
