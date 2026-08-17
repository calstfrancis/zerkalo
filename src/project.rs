use std::path::{Path, PathBuf};

/// Recursively collect all `.typ` files under `root`, respecting `.gitignore`.
pub fn collect_typ_files(root: &Path) -> Vec<PathBuf> {
    let repo = git2::Repository::open(root).ok();
    let mut files = Vec::new();
    collect_recursive(root, &repo, &mut files);
    files.sort();
    files
}

/// Extracts the `.typ` paths a document's `#import`/`#include` statements
/// reference, resolved relative to `base_dir` and canonicalized. Shared by
/// the dependency graph and the manuscript-wide outline, so both walk the
/// include graph the same way.
pub(crate) fn parse_typ_imports(content: &str, base_dir: &Path) -> Vec<PathBuf> {
    let re = regex::Regex::new(r#"#(?:import|include)\s+"([^"]+\.typ)""#).unwrap();
    re.captures_iter(content)
        .filter_map(|c| {
            let raw = c[1].to_string();
            let full = base_dir.join(&raw);
            let canonical = std::fs::canonicalize(&full).unwrap_or(full);
            if canonical.exists() { Some(canonical) } else { None }
        })
        .collect()
}

/// Walks the `#include`/`#import` graph from `root` breadth-first, returning
/// each visited file's content in visitation order (root first) — the shape
/// a manuscript-wide outline wants, as opposed to `collect_typ_files`'s
/// alphabetical directory listing. Files that can't be read are skipped,
/// not treated as fatal — a broken `#include` shouldn't blank the whole view.
pub fn manuscript_files(root: &Path, project_root: &Path) -> Vec<(PathBuf, String)> {
    let mut out = Vec::new();
    let mut queue: std::collections::VecDeque<PathBuf> = std::collections::VecDeque::new();
    let mut visited: std::collections::HashSet<PathBuf> = std::collections::HashSet::new();

    let canonical_root = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    queue.push_back(canonical_root);

    while let Some(path) = queue.pop_front() {
        if visited.contains(&path) {
            continue;
        }
        visited.insert(path.clone());

        let Ok(content) = std::fs::read_to_string(&path) else { continue };
        let base = path.parent().unwrap_or(project_root);
        for child in parse_typ_imports(&content, base) {
            if !visited.contains(&child) {
                queue.push_back(child);
            }
        }
        out.push((path, content));
    }
    out
}

fn collect_recursive(
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
            collect_recursive(&path, repo, out);
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

    #[test]
    fn manuscript_files_visits_root_first_then_includes_in_order() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("main.typ");
        std::fs::write(&root, "#include \"ch1.typ\"\n#include \"ch2.typ\"\n").unwrap();
        std::fs::write(dir.path().join("ch1.typ"), "= Chapter One").unwrap();
        std::fs::write(dir.path().join("ch2.typ"), "= Chapter Two").unwrap();

        let files = manuscript_files(&root, dir.path());
        let names: Vec<String> = files.iter()
            .map(|(p, _)| p.file_name().unwrap().to_string_lossy().to_string())
            .collect();
        assert_eq!(names, vec!["main.typ", "ch1.typ", "ch2.typ"]);
    }

    #[test]
    fn manuscript_files_does_not_loop_forever_on_a_cycle() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.typ");
        let b = dir.path().join("b.typ");
        std::fs::write(&a, "#include \"b.typ\"").unwrap();
        std::fs::write(&b, "#include \"a.typ\"").unwrap();

        let files = manuscript_files(&a, dir.path());
        assert_eq!(files.len(), 2);
    }

    #[test]
    fn manuscript_files_skips_a_missing_include_without_failing() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("main.typ");
        std::fs::write(&root, "#include \"missing.typ\"\n= Only Heading").unwrap();

        let files = manuscript_files(&root, dir.path());
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].0.file_name().unwrap(), "main.typ");
    }

    #[test]
    fn manuscript_files_single_file_project_returns_just_that_file() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("main.typ");
        std::fs::write(&root, "= Only Heading").unwrap();

        let files = manuscript_files(&root, dir.path());
        assert_eq!(files.len(), 1);
    }
}
