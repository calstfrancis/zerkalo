use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use regex::Regex;

static IMPORT_RE: OnceLock<Regex> = OnceLock::new();

fn import_regex() -> &'static Regex {
    IMPORT_RE.get_or_init(|| {
        Regex::new(r#"#\s*(?:import|include)\s+"([^"]+\.typ)""#).unwrap()
    })
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
        let files = crate::project::collect_typ_files(&root);
        let imports = build_import_graph(&files);
        let root_file = if let Some(cfg) = crate::config::ProjectConfig::load(&root) {
            if let Some(rel) = cfg.root_file {
                // Config wins unconditionally; fall back to detect_root if path is bad
                let abs = root.join(&rel);
                if abs.exists() { Some(abs) } else { detect_root(&files, &imports) }
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
