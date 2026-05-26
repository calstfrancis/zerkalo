use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Default, Clone)]
pub struct Session {
    pub open_files: Vec<PathBuf>,
    pub active_file: Option<PathBuf>,
    /// Cursor offsets (byte offset in buffer) per file
    pub cursor_positions: HashMap<PathBuf, i32>,
}

impl Session {
    pub fn load() -> Self {
        let path = session_path();
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        let path = session_path();
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(path, json);
        }
    }
}

fn session_path() -> PathBuf {
    let base = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    PathBuf::from(base).join(".local/share/zerkalo/session.json")
}
