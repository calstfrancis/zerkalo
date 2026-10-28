//! Coming back to a document: the quiet "last time" line and the note to self.

use std::path::{Path, PathBuf};

use chrono::{DateTime, Local};

use crate::writing_log::WritingLog;

/// "Last time: Tuesday 14:20 · 340 words added", or nothing for a document
/// that has never been written in here.
pub fn last_time_line(path: &Path, log: &WritingLog, now: DateTime<Local>) -> Option<String> {
    let modified: DateTime<Local> = std::fs::metadata(path).ok()?.modified().ok()?.into();
    let when = when_phrase(modified, now);
    let added = log
        .sessions
        .iter()
        .rev()
        .find(|s| s.file == path && s.words_added > 0)
        .map(|s| s.words_added);
    Some(match added {
        Some(n) => format!(
            "Last time: {when} \u{b7} {n} {} added",
            if n == 1 { "word" } else { "words" }
        ),
        None => format!("Last time: {when}"),
    })
}

fn when_phrase(then: DateTime<Local>, now: DateTime<Local>) -> String {
    let days = (now.date_naive() - then.date_naive()).num_days();
    match days {
        0 => format!("today {}", then.format("%H:%M")),
        1 => format!("yesterday {}", then.format("%H:%M")),
        2..=6 => then.format("%A %H:%M").to_string(),
        _ => then.format("%-d %B %Y").to_string(),
    }
}

fn note_path(doc: &Path) -> PathBuf {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    doc.hash(&mut h);
    crate::config::zerkalo_data_dir()
        .join("notes")
        .join(format!("{:016x}.txt", h.finish()))
}

pub fn load_note(doc: &Path) -> String {
    std::fs::read_to_string(note_path(doc))
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}

pub fn save_note(doc: &Path, note: &str) {
    let p = note_path(doc);
    if note.trim().is_empty() {
        let _ = std::fs::remove_file(p);
        return;
    }
    if let Some(dir) = p.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = crate::error::atomic_write(&p, note.trim().as_bytes());
}

/// "34 min" or "1 h 20" for how long a session has run.
pub fn clock(secs: u64) -> String {
    let mins = secs / 60;
    if mins < 60 {
        format!("{mins} min")
    } else {
        format!("{} h {:02}", mins / 60, mins % 60)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn phrases_the_day() {
        let now = Local.with_ymd_and_hms(2026, 10, 28, 9, 0, 0).unwrap();
        let at = |d, h| Local.with_ymd_and_hms(2026, 10, d, h, 20, 0).unwrap();
        assert_eq!(when_phrase(at(28, 7), now), "today 07:20");
        assert_eq!(when_phrase(at(27, 14), now), "yesterday 14:20");
        assert_eq!(when_phrase(at(24, 14), now), "Saturday 14:20");
        assert_eq!(when_phrase(at(2, 14), now), "2 October 2026");
    }

    #[test]
    fn clock_is_plain() {
        assert_eq!(clock(5 * 60), "5 min");
        assert_eq!(clock(80 * 60), "1 h 20");
    }
}
