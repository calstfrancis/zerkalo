use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Default, Clone)]
pub struct CompileStats {
    pub total_compiles: u64,
    pub total_ms: u64,
    pub slow_count: u64,
    pub last_ms: u64,
}

impl CompileStats {
    // Read side of the compile-stats module: `record()` is called on every compile and persists
    // to disk, but nothing displays the aggregated stats yet (no stats UI exists). Kept as the
    // natural read API for whenever that lands, rather than collecting data nothing can read.
    #[allow(dead_code)]
    pub fn average_ms(&self) -> f64 {
        if self.total_compiles == 0 {
            0.0
        } else {
            self.total_ms as f64 / self.total_compiles as f64
        }
    }
}

static CACHE: OnceLock<Mutex<CompileStats>> = OnceLock::new();

fn cache() -> &'static Mutex<CompileStats> {
    CACHE.get_or_init(|| Mutex::new(load_from_disk()))
}

fn stats_path() -> PathBuf {
    let base = shellexpand::tilde("~/.cache/zerkalo").into_owned();
    PathBuf::from(base).join("compile_stats.json")
}

fn load_from_disk() -> CompileStats {
    let path = stats_path();
    if let Ok(text) = std::fs::read_to_string(&path) {
        if let Ok(s) = serde_json::from_str(&text) {
            return s;
        }
    }
    CompileStats::default()
}

// See `average_ms`'s comment above — same "write path exists, read path doesn't yet" state.
#[allow(dead_code)]
pub fn load() -> CompileStats {
    // Recover from poisoning rather than panic — a panic elsewhere while
    // holding this lock shouldn't turn every subsequent stats read/write
    // into a second panic for the life of the process. Stats are
    // best-effort telemetry, not data whose consistency matters that much.
    cache().lock().unwrap_or_else(|e| e.into_inner()).clone()
}

pub fn record(ms: u64) {
    let mut stats = cache().lock().unwrap_or_else(|e| e.into_inner());
    stats.total_compiles += 1;
    stats.total_ms += ms;
    stats.last_ms = ms;
    if ms >= 3000 {
        stats.slow_count += 1;
    }
    // Flush to disk every 10 compiles to amortise I/O.
    if stats.total_compiles.is_multiple_of(10) {
        let snap = stats.clone();
        drop(stats);
        flush_to_disk(&snap);
    }
}

fn flush_to_disk(stats: &CompileStats) {
    let path = stats_path();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(json) = serde_json::to_string(stats) {
        let _ = crate::error::atomic_write(&path, json.as_bytes());
    }
}
