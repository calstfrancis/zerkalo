//! A private record, kept on this computer, of when writing happened in a
//! document: the stretches of typing, and any text pasted in. It exists so the
//! writing history (`writing_history`) can show more than saved versions —
//! how long someone actually wrote, and how much of the text was typed rather
//! than pasted.
//!
//! Only counts and times are kept — never the text itself. One file per
//! project, written about every half minute while writing.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use chrono::{DateTime, Duration, FixedOffset, Local};
use serde::{Deserialize, Serialize};

/// A pause longer than this ends a stretch of writing.
const GAP: Duration = Duration::minutes(10);
/// Time between two keystrokes counts as writing only up to this long, so a
/// pause to think is not mistaken for time away.
const THINKING: Duration = Duration::minutes(2);
/// Pastes shorter than this (in characters) are not worth listing.
pub const PASTE_NOTED: usize = 40;

/// Times are kept as text, as `2026-03-01T10:00:00+01:00`.
mod rfc3339 {
    use chrono::{DateTime, FixedOffset};
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(t: &DateTime<FixedOffset>, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&t.to_rfc3339())
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<DateTime<FixedOffset>, D::Error> {
        let text = String::deserialize(d)?;
        DateTime::parse_from_rfc3339(&text).map_err(serde::de::Error::custom)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Session {
    #[serde(with = "rfc3339")]
    pub start: DateTime<FixedOffset>,
    #[serde(with = "rfc3339")]
    pub end: DateTime<FixedOffset>,
    /// Time spent actually writing, in seconds.
    pub active_secs: i64,
    pub typed_chars: usize,
    pub pasted_chars: usize,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Paste {
    #[serde(with = "rfc3339")]
    pub when: DateTime<FixedOffset>,
    pub file: String,
    pub chars: usize,
    pub words: usize,
}

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub struct ActivityLog {
    #[serde(default)]
    pub sessions: Vec<Session>,
    #[serde(default)]
    pub pastes: Vec<Paste>,
}

impl ActivityLog {
    pub fn load(document: &Path) -> Self {
        std::fs::read_to_string(log_path(document))
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    fn save(&self, document: &Path) {
        let path = log_path(document);
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).ok();
        }
        if let Ok(s) = serde_json::to_string(self) {
            crate::error::atomic_write(&path, s.as_bytes()).ok();
        }
    }

    /// Notes `chars` typed at `now`, continuing the current session or
    /// starting a new one after a long pause.
    fn typed_at(&mut self, now: DateTime<FixedOffset>, chars: usize) {
        match self.sessions.last_mut() {
            Some(s) if now - s.end <= GAP => {
                let gap = now - s.end;
                s.active_secs += gap.min(THINKING).num_seconds();
                s.end = now;
                s.typed_chars += chars;
            }
            _ => self.sessions.push(Session {
                start: now,
                end: now,
                active_secs: 0,
                typed_chars: chars,
                pasted_chars: 0,
            }),
        }
    }

    fn pasted_at(&mut self, now: DateTime<FixedOffset>, file: String, chars: usize, words: usize) {
        match self.sessions.last_mut() {
            Some(s) if now - s.end <= GAP => {
                s.end = now;
                s.pasted_chars += chars;
            }
            _ => self.sessions.push(Session {
                start: now,
                end: now,
                active_secs: 0,
                typed_chars: 0,
                pasted_chars: chars,
            }),
        }
        if chars >= PASTE_NOTED {
            self.pastes.push(Paste {
                when: now,
                file,
                chars,
                words,
            });
        }
    }

    pub fn is_empty(&self) -> bool {
        self.sessions.is_empty() && self.pastes.is_empty()
    }
}

fn log_path(document: &Path) -> PathBuf {
    let canonical = std::fs::canonicalize(document).unwrap_or_else(|_| document.to_path_buf());
    crate::config::zerkalo_data_dir()
        .join("activity")
        .join(format!("{}.json", crate::auto_save::path_key(&canonical)))
}

struct Recorder {
    project: Option<PathBuf>,
    logs: HashMap<PathBuf, ActivityLog>,
    dirty: HashSet<PathBuf>,
    pasting: bool,
}

thread_local! {
    static RECORDER: RefCell<Recorder> = RefCell::new(Recorder {
        project: None,
        logs: HashMap::new(),
        dirty: HashSet::new(),
        pasting: false,
    });
}

fn now() -> DateTime<FixedOffset> {
    Local::now().fixed_offset()
}

/// Starts recording in `project`. Each document keeps its own record, picked
/// up from earlier sessions the first time it is written in.
pub fn begin(project: &Path) {
    RECORDER.with(|r| {
        let mut r = r.borrow_mut();
        r.project = Some(project.to_path_buf());
        r.logs.clear();
        r.dirty.clear();
    });
}

/// The next insertion is a paste, until `paste_finished`.
pub fn paste_starting() {
    RECORDER.with(|r| r.borrow_mut().pasting = true);
}

pub fn paste_finished() {
    RECORDER.with(|r| r.borrow_mut().pasting = false);
}

/// Text of `text` was just inserted into `file`.
pub fn inserted(file: &Path, text: &str) {
    RECORDER.with(|r| {
        let mut r = r.borrow_mut();
        if r.project.is_none() {
            return;
        }
        let chars = text.chars().count();
        let pasting = r.pasting;
        if !pasting && chars > 4 {
            return;
        }
        let rel = r
            .project
            .as_deref()
            .and_then(|p| file.strip_prefix(p).ok())
            .unwrap_or(file)
            .to_string_lossy()
            .into_owned();
        let log = r
            .logs
            .entry(file.to_path_buf())
            .or_insert_with(|| ActivityLog::load(file));
        if pasting {
            log.pasted_at(now(), rel, chars, text.split_whitespace().count());
        } else {
            log.typed_at(now(), chars);
        }
        r.dirty.insert(file.to_path_buf());
    });
}

/// Writes out the record of every document that changed since the last time.
pub fn flush() {
    RECORDER.with(|r| {
        let mut r = r.borrow_mut();
        for file in std::mem::take(&mut r.dirty) {
            if let Some(log) = r.logs.get(&file) {
                log.save(&file);
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(s: &str) -> DateTime<FixedOffset> {
        DateTime::parse_from_rfc3339(s).unwrap()
    }

    #[test]
    fn typing_close_together_is_one_session_and_a_long_pause_starts_another() {
        let mut log = ActivityLog::default();
        log.typed_at(at("2026-03-01T10:00:00+00:00"), 1);
        log.typed_at(at("2026-03-01T10:00:30+00:00"), 1);
        log.typed_at(at("2026-03-01T10:05:00+00:00"), 1);
        log.typed_at(at("2026-03-01T11:00:00+00:00"), 1);
        assert_eq!(log.sessions.len(), 2);
        assert_eq!(log.sessions[0].typed_chars, 3);
        // 30 s, then a 4½ minute pause counts as 2 minutes of thinking.
        assert_eq!(log.sessions[0].active_secs, 30 + 120);
    }

    #[test]
    fn only_larger_pastes_are_listed_but_all_are_counted() {
        let mut log = ActivityLog::default();
        log.pasted_at(at("2026-03-01T10:00:00+00:00"), "a.typ".into(), 10, 2);
        log.pasted_at(at("2026-03-01T10:01:00+00:00"), "a.typ".into(), 400, 70);
        assert_eq!(log.pastes.len(), 1);
        assert_eq!(log.sessions[0].pasted_chars, 410);
    }
}
