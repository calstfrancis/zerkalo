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

        let mut streak = 0u32;
        let mut day = chrono::Local::now().date_naive();
        loop {
            let ds = day.format("%Y-%m-%d").to_string();
            if active_days.contains(&ds) {
                streak += 1;
                day = match day.pred_opt() {
                    Some(d) => d,
                    None => break,
                };
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
