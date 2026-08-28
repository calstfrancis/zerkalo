#![allow(dead_code)]

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use regex::Regex;

static IMPORT_RE: OnceLock<Regex> = OnceLock::new();

fn import_regex() -> &'static Regex {
    IMPORT_RE.get_or_init(|| Regex::new(r#"#\s*(?:import|include)\s+"([^"]+\.typ)""#).unwrap())
}

/// Resolves a `.zerkalo/config.toml` `root_file` value against the project
/// root, rejecting anything that escapes it (an absolute path or a "../"
/// traversal in a shared/cloned config must not be able to point the
/// compiler at arbitrary files on disk). Returns `None` if the path is
/// missing or escapes `root`.
pub fn resolve_root_file(root: &Path, rel: &Path) -> Option<PathBuf> {
    let abs = root.join(rel);
    let canonical_root = std::fs::canonicalize(root).ok()?;
    let canonical = std::fs::canonicalize(&abs).ok()?;
    if canonical.starts_with(&canonical_root) {
        Some(canonical)
    } else {
        None
    }
}

#[allow(dead_code)]
pub struct ProjectModel {
    pub root: PathBuf,
    /// Detected compilation root (the file that is not imported by any other).
    pub root_file: Option<PathBuf>,
    /// Import graph: file → list of files it imports.
    pub imports: HashMap<PathBuf, Vec<PathBuf>>,
}

impl ProjectModel {
    /// Files that are not imported by any other file — valid compilation roots.
    pub fn candidate_roots(&self) -> Vec<PathBuf> {
        let imported: std::collections::HashSet<&PathBuf> =
            self.imports.values().flatten().collect();
        let mut candidates: Vec<PathBuf> = self
            .imports
            .keys()
            .filter(|f| !imported.contains(f))
            .cloned()
            .collect();
        candidates.sort();
        candidates
    }

    pub fn scan(root: PathBuf) -> Self {
        // Canonicalize once so every path flowing through (file list, import
        // targets) is in the same form — parse_imports() canonicalizes each
        // import target it resolves, so a non-canonical root here would make
        // `imports.keys()` (raw) and `imports.values()` (canonical) disagree,
        // causing candidate_roots()/detect_root() to miss real imports.
        let root = std::fs::canonicalize(&root).unwrap_or(root);
        let files = crate::project::collect_typ_files(&root);
        let imports = build_import_graph(&files);
        let root_file = if let Some(cfg) = crate::config::ProjectConfig::load(&root) {
            if let Some(rel) = cfg.root_file {
                // Config wins unconditionally; fall back to detect_root if the
                // path is bad or escapes the project directory.
                resolve_root_file(&root, &rel).or_else(|| detect_root(&files, &imports))
            } else {
                detect_root(&files, &imports)
            }
        } else {
            detect_root(&files, &imports)
        };
        Self {
            root,
            root_file,
            imports,
        }
    }
}

fn build_import_graph(files: &[PathBuf]) -> HashMap<PathBuf, Vec<PathBuf>> {
    files
        .iter()
        .map(|f| (f.clone(), parse_imports(f)))
        .collect()
}

/// Parse `#import "..."` and `#include "..."` from a `.typ` file.
/// Returns absolute paths to imported `.typ` files that exist on disk.
fn parse_imports(file: &Path) -> Vec<PathBuf> {
    let content = match std::fs::read_to_string(file) {
        Ok(c) => c,
        Err(_) => return vec![],
    };
    let dir = file.parent().unwrap_or(Path::new("."));
    import_regex()
        .captures_iter(&content)
        .filter_map(|cap| {
            let rel = cap.get(1)?.as_str();
            std::fs::canonicalize(dir.join(rel)).ok()
        })
        .collect()
}

/// The root file is the `.typ` file that no other file imports.
/// If all files import each other (cycles) or there's only one, pick the
/// largest by byte size.
fn detect_root(files: &[PathBuf], imports: &HashMap<PathBuf, Vec<PathBuf>>) -> Option<PathBuf> {
    if files.is_empty() {
        return None;
    }
    if files.len() == 1 {
        return Some(files[0].clone());
    }

    let imported: HashSet<&PathBuf> = imports.values().flatten().collect();

    let mut candidates: Vec<&PathBuf> = files.iter().filter(|f| !imported.contains(*f)).collect();

    // Fall back to all files if everything is imported (circular or flat project)
    if candidates.is_empty() {
        candidates = files.iter().collect();
    }

    candidates
        .into_iter()
        .max_by_key(|f| std::fs::metadata(f).map(|m| m.len()).unwrap_or(0))
        .cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_imports_finds_import_and_include() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("chapter.typ"), "content").unwrap();
        let main = dir.path().join("main.typ");
        std::fs::write(
            &main,
            "#import \"chapter.typ\": *\n#include \"chapter.typ\"\n",
        )
        .unwrap();

        let imports = parse_imports(&main);
        let expected = std::fs::canonicalize(dir.path().join("chapter.typ")).unwrap();
        assert_eq!(
            imports.len(),
            2,
            "both #import and #include should match, no dedup here"
        );
        assert!(imports.iter().all(|p| p == &expected));
    }

    #[test]
    fn parse_imports_ignores_nonexistent_targets() {
        let dir = tempfile::tempdir().unwrap();
        let main = dir.path().join("main.typ");
        std::fs::write(&main, "#import \"missing.typ\": *\n").unwrap();
        assert!(parse_imports(&main).is_empty());
    }

    #[test]
    fn parse_imports_missing_file_returns_empty() {
        let path = std::path::PathBuf::from("/tmp/zerkalo-nonexistent-file-xyz.typ");
        assert!(parse_imports(&path).is_empty());
    }

    #[test]
    fn detect_root_picks_the_only_file() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("solo.typ");
        std::fs::write(&f, "content").unwrap();
        let files = vec![f.clone()];
        let imports = HashMap::new();
        assert_eq!(detect_root(&files, &imports), Some(f));
    }

    #[test]
    fn detect_root_returns_none_for_no_files() {
        assert_eq!(detect_root(&[], &HashMap::new()), None);
    }

    #[test]
    fn detect_root_picks_the_file_not_imported_by_others() {
        let dir = tempfile::tempdir().unwrap();
        let main = dir.path().join("main.typ");
        let chapter = dir.path().join("chapter.typ");
        std::fs::write(&main, "x").unwrap();
        std::fs::write(&chapter, "y").unwrap();

        let files = vec![main.clone(), chapter.clone()];
        let mut imports = HashMap::new();
        imports.insert(main.clone(), vec![chapter.clone()]);
        imports.insert(chapter.clone(), vec![]);

        assert_eq!(detect_root(&files, &imports), Some(main));
    }

    #[test]
    fn detect_root_falls_back_to_largest_file_on_cycle() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.typ");
        let b = dir.path().join("b.typ");
        std::fs::write(&a, "short").unwrap();
        std::fs::write(&b, "much much longer content here").unwrap();

        let files = vec![a.clone(), b.clone()];
        let mut imports = HashMap::new();
        imports.insert(a.clone(), vec![b.clone()]);
        imports.insert(b.clone(), vec![a.clone()]);

        assert_eq!(detect_root(&files, &imports), Some(b));
    }

    #[cfg(unix)]
    #[test]
    fn scan_through_a_symlinked_root_still_detects_the_correct_compile_root() {
        // Reproduces the canonicalization mismatch: parse_imports() always
        // canonicalizes resolved import targets, so scanning via a symlinked
        // root used to leave `files`/`imports.keys()` in a different (raw)
        // path form than `imports.values()`, making candidate_roots() think
        // chapter.typ was never imported by anyone.
        let real_dir = tempfile::tempdir().unwrap();
        std::fs::write(real_dir.path().join("chapter.typ"), "content").unwrap();
        std::fs::write(
            real_dir.path().join("main.typ"),
            "#import \"chapter.typ\": *\n",
        )
        .unwrap();

        let parent = tempfile::tempdir().unwrap();
        let link_path = parent.path().join("link");
        std::os::unix::fs::symlink(real_dir.path(), &link_path).unwrap();

        let model = ProjectModel::scan(link_path.clone());
        let candidates = model.candidate_roots();
        assert_eq!(
            candidates.len(),
            1,
            "chapter.typ should be recognized as imported and excluded; got {candidates:?}"
        );
        assert_eq!(candidates[0].file_name().unwrap(), "main.typ");
    }

    #[test]
    fn candidate_roots_excludes_imported_files() {
        let dir = tempfile::tempdir().unwrap();
        let main = dir.path().join("main.typ");
        let chapter = dir.path().join("chapter.typ");
        std::fs::write(&main, "x").unwrap();
        std::fs::write(&chapter, "y").unwrap();

        let mut imports = HashMap::new();
        imports.insert(main.clone(), vec![chapter.clone()]);
        imports.insert(chapter.clone(), vec![]);

        let model = ProjectModel {
            root: dir.path().to_path_buf(),
            root_file: None,
            imports,
        };
        assert_eq!(model.candidate_roots(), vec![main]);
    }
}
