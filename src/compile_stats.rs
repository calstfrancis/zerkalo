use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Default, Clone)]
pub struct CompileStats {
    pub total_compiles: u64,
    pub total_ms: u64,
    pub slow_count: u64,
    pub last_ms: u64,
}

impl CompileStats {
    #[allow(dead_code)]
    pub fn average_ms(&self) -> f64 {
        if self.total_compiles == 0 {
            0.0
        } else {
            self.total_ms as f64 / self.total_compiles as f64
        }
    }
}

fn stats_path() -> PathBuf {
    let base = shellexpand::tilde("~/.cache/zerkalo").into_owned();
    PathBuf::from(base).join("compile_stats.json")
}

pub fn load() -> CompileStats {
    let path = stats_path();
    if let Ok(text) = std::fs::read_to_string(&path) {
        if let Ok(s) = serde_json::from_str(&text) {
            return s;
        }
    }
    CompileStats::default()
}

pub fn record(ms: u64) {
    let mut stats = load();
    stats.total_compiles += 1;
    stats.total_ms += ms;
    stats.last_ms = ms;
    if ms >= 3000 {
        stats.slow_count += 1;
    }
    let path = stats_path();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(json) = serde_json::to_string(&stats) {
        let _ = std::fs::write(path, json);
    }
}
