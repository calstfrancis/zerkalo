use std::collections::{BTreeSet, HashMap};
use std::path::PathBuf;

use chrono::Datelike;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Default, Clone)]
pub struct WritingSession {
    pub date: String,
    pub file: PathBuf,
    pub words_added: i32,
    pub duration_secs: u64,
}

#[derive(Serialize, Deserialize, Default, Clone)]
pub struct WritingLog {
    pub sessions: Vec<WritingSession>,
}

impl WritingLog {
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

    pub fn record(&mut self, file: PathBuf, words_added: i32, duration_secs: u64) {
        let date = today_str();
        self.sessions.push(WritingSession { date, file, words_added, duration_secs });
        self.save();
    }

    pub fn total_today(&self) -> i32 {
        let today = today_str();
        self.sessions.iter()
            .filter(|s| s.date == today)
            .map(|s| s.words_added)
            .sum()
    }

    pub fn total_this_week(&self) -> i32 {
        let now = chrono::Local::now();
        let days_from_monday = now.weekday().num_days_from_monday() as i64;
        let week_start = (now - chrono::Duration::days(days_from_monday))
            .format("%Y-%m-%d")
            .to_string();
        self.sessions.iter()
            .filter(|s| s.date.as_str() >= week_start.as_str())
            .map(|s| s.words_added)
            .sum()
    }

    pub fn streak_days(&self) -> u32 {
        let active_days: BTreeSet<String> = self.sessions.iter()
            .filter(|s| s.words_added > 0)
            .map(|s| s.date.clone())
            .collect();

        let mut day = chrono::Local::now().date_naive();
        let today = day.format("%Y-%m-%d").to_string();
        // If nothing written yet today, let the streak survive until midnight
        // by starting the count from yesterday instead of breaking immediately.
        if !active_days.contains(&today) {
            day = match day.pred_opt() { Some(d) => d, None => return 0 };
        }
        let mut streak = 0u32;
        loop {
            let ds = day.format("%Y-%m-%d").to_string();
            if active_days.contains(&ds) {
                streak += 1;
                day = match day.pred_opt() { Some(d) => d, None => break };
            } else {
                break;
            }
        }
        streak
    }
}

// Count whitespace-delimited words in a string.
pub fn count_words(text: &str) -> i32 {
    text.split_whitespace().count() as i32
}

// Track per-file word counts at session start so we can compute diffs on close.
pub type FileStartWords = std::rc::Rc<std::cell::RefCell<HashMap<PathBuf, i32>>>;

pub fn new_file_start_words() -> FileStartWords {
    std::rc::Rc::new(std::cell::RefCell::new(HashMap::new()))
}

fn today_str() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
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
    base.join("zerkalo").join("writing_log.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session(date: &str, words: i32) -> WritingSession {
        WritingSession { date: date.to_string(), file: PathBuf::from("main.typ"), words_added: words, duration_secs: 60 }
    }

    #[test]
    fn count_words_splits_on_whitespace() {
        assert_eq!(count_words("hello   world\nfoo"), 3);
        assert_eq!(count_words(""), 0);
        assert_eq!(count_words("   "), 0);
    }

    #[test]
    fn total_today_sums_only_todays_sessions() {
        let today = today_str();
        let log = WritingLog { sessions: vec![
            session(&today, 100),
            session(&today, 50),
            session("2000-01-01", 999),
        ]};
        assert_eq!(log.total_today(), 150);
    }

    #[test]
    fn total_today_zero_when_no_sessions_today() {
        let log = WritingLog { sessions: vec![session("2000-01-01", 999)] };
        assert_eq!(log.total_today(), 0);
    }

    #[test]
    fn total_this_week_excludes_sessions_before_week_start() {
        let log = WritingLog { sessions: vec![
            session(&today_str(), 40),
            session("2000-01-01", 999),
        ]};
        assert_eq!(log.total_this_week(), 40);
    }

    #[test]
    fn streak_days_counts_consecutive_active_days_including_today() {
        let today = chrono::Local::now().date_naive();
        let d0 = today.format("%Y-%m-%d").to_string();
        let d1 = today.pred_opt().unwrap().format("%Y-%m-%d").to_string();
        let d2 = today.pred_opt().unwrap().pred_opt().unwrap().format("%Y-%m-%d").to_string();
        let log = WritingLog { sessions: vec![session(&d0, 10), session(&d1, 5), session(&d2, 5)] };
        assert_eq!(log.streak_days(), 3);
    }

    #[test]
    fn streak_days_survives_zero_words_today_by_counting_from_yesterday() {
        let today = chrono::Local::now().date_naive();
        let d1 = today.pred_opt().unwrap().format("%Y-%m-%d").to_string();
        let log = WritingLog { sessions: vec![session(&d1, 5)] };
        assert_eq!(log.streak_days(), 1);
    }

    #[test]
    fn streak_days_breaks_on_gap() {
        let today = chrono::Local::now().date_naive();
        let d0 = today.format("%Y-%m-%d").to_string();
        let d2 = today.pred_opt().unwrap().pred_opt().unwrap().format("%Y-%m-%d").to_string();
        // Yesterday (d1) is missing, so the streak should stop at today.
        let log = WritingLog { sessions: vec![session(&d0, 10), session(&d2, 5)] };
        assert_eq!(log.streak_days(), 1);
    }

    #[test]
    fn streak_days_ignores_sessions_with_zero_words() {
        let today = chrono::Local::now().date_naive();
        let d0 = today.format("%Y-%m-%d").to_string();
        let log = WritingLog { sessions: vec![session(&d0, 0)] };
        assert_eq!(log.streak_days(), 0);
    }
}
