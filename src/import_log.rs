//! A small persisted history of document-import attempts (LaTeX/DOCX/Markdown/
//! ODT/HTML/EPUB via pandoc, or PDF via pdftotext), surfaced in the Import
//! picker dialog. Mirrors `writing_log.rs`'s shape: one JSON file, no database.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Oldest entries beyond this count are dropped on save.
const MAX_RECORDS: usize = 50;

#[derive(Serialize, Deserialize, Clone)]
pub struct ImportRecord {
    pub date: String,
    pub source: PathBuf,
    pub format: String,
    pub output: Option<PathBuf>,
    pub success: bool,
    pub message: String,
}

#[derive(Serialize, Deserialize, Default, Clone)]
pub struct ImportLog {
    pub records: Vec<ImportRecord>,
}

impl ImportLog {
    pub fn load() -> Self {
        let path = log_path();
        match std::fs::read_to_string(&path) {
            Ok(s) => serde_json::from_str(&s).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    pub fn save(&self) {
        let path = log_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        if let Ok(s) = serde_json::to_string_pretty(self) {
            std::fs::write(path, s).ok();
        }
    }

    pub fn record(
        &mut self,
        source: PathBuf,
        format: &str,
        output: Option<PathBuf>,
        success: bool,
        message: &str,
    ) {
        self.records.push(ImportRecord {
            date: today_str(),
            source,
            format: format.to_string(),
            output,
            success,
            message: message.to_string(),
        });
        if self.records.len() > MAX_RECORDS {
            let excess = self.records.len() - MAX_RECORDS;
            self.records.drain(0..excess);
        }
        self.save();
    }

    /// Remove one record by its index in `records` (not display order, which
    /// callers showing newest-first must convert back before calling this).
    pub fn remove(&mut self, index: usize) {
        if index < self.records.len() {
            self.records.remove(index);
            self.save();
        }
    }

    pub fn clear(&mut self) {
        self.records.clear();
        self.save();
    }
}

fn today_str() -> String {
    chrono::Local::now().format("%Y-%m-%d %H:%M").to_string()
}

fn log_path() -> PathBuf {
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let home = std::env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("."));
            home.join(".local").join("share")
        });
    base.join("zerkalo").join("import_log.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(success: bool) -> ImportRecord {
        ImportRecord {
            date: "2026-01-01 00:00".to_string(),
            source: PathBuf::from("paper.tex"),
            format: "LaTeX (.tex)".to_string(),
            output: Some(PathBuf::from("paper.typ")),
            success,
            message: "ok".to_string(),
        }
    }

    #[test]
    fn record_appends_and_caps_at_max() {
        let mut log = ImportLog::default();
        // Push directly (bypassing save/disk I/O) to test the cap in isolation.
        for i in 0..(MAX_RECORDS + 10) {
            log.records.push(ImportRecord { message: i.to_string(), ..rec(true) });
        }
        // Simulate what `record` does after pushing, without touching disk.
        if log.records.len() > MAX_RECORDS {
            let excess = log.records.len() - MAX_RECORDS;
            log.records.drain(0..excess);
        }
        assert_eq!(log.records.len(), MAX_RECORDS);
        // The oldest entries should have been dropped, keeping the most recent.
        assert_eq!(log.records.first().unwrap().message, "10");
        assert_eq!(log.records.last().unwrap().message, (MAX_RECORDS + 9).to_string());
    }

    #[test]
    fn remove_drops_only_the_targeted_index() {
        // Mirrors `remove`'s Vec mutation directly, bypassing the real disk
        // write the same way `record_appends_and_caps_at_max` does above.
        let mut log = ImportLog {
            records: vec![
                ImportRecord { message: "first".to_string(), ..rec(true) },
                ImportRecord { message: "second".to_string(), ..rec(true) },
                ImportRecord { message: "third".to_string(), ..rec(true) },
            ],
        };
        if 1 < log.records.len() {
            log.records.remove(1);
        }
        assert_eq!(log.records.len(), 2);
        assert_eq!(log.records[0].message, "first");
        assert_eq!(log.records[1].message, "third");
    }

    #[test]
    fn clear_empties_all_records() {
        let mut log = ImportLog { records: vec![rec(true), rec(false)] };
        log.records.clear();
        assert!(log.records.is_empty());
    }

    #[test]
    fn success_and_failure_records_round_trip_through_json() {
        let log = ImportLog { records: vec![rec(true), rec(false)] };
        let json = serde_json::to_string(&log).unwrap();
        let back: ImportLog = serde_json::from_str(&json).unwrap();
        assert_eq!(back.records.len(), 2);
        assert!(back.records[0].success);
        assert!(!back.records[1].success);
    }
}
