//! Filesystem-scan helpers backing the header's "Open" dropdown fallback
//! list (recent files plus anything else found on disk).
//!
//! Used to also host a `DocsBrowser` window ("Browse Documents…" in the
//! hamburger menu) — removed because it was a strictly thinner, unfiltered
//! duplicate of the Library window (Ctrl+L), which is database-backed, kept
//! in sync with the filesystem on every launch (`Library::import_directory`),
//! and additionally offers search, project/category/tag filters, sort, pin,
//! and bulk actions that this file never had. Nothing distinguished the two
//! to a user, and Library is a strict superset for the common case.

use std::path::PathBuf;
use std::time::SystemTime;

pub fn scan_typ_files(dir: &PathBuf, depth: usize) -> Vec<(PathBuf, SystemTime)> {
    let mut files = Vec::new();
    if depth == 0 {
        return files;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return files;
    };
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_dir() {
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if !name.starts_with('.') {
                files.extend(scan_typ_files(&p, depth - 1));
            }
        } else if p.extension().and_then(|e| e.to_str()) == Some("typ") {
            let mtime = std::fs::metadata(&p)
                .and_then(|m| m.modified())
                .unwrap_or(SystemTime::UNIX_EPOCH);
            files.push((p, mtime));
        }
    }
    files
}
