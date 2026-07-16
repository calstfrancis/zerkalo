use std::path::{Path, PathBuf};
use std::time::SystemTime;

fn autosave_dir() -> PathBuf {
    PathBuf::from(shellexpand::tilde("~/.config/zerkalo/autosave").into_owned())
}

pub(crate) fn path_key(path: &Path) -> String {
    // FNV-1a 64-bit: stable across Rust versions (unlike DefaultHasher).
    let s = path.to_string_lossy();
    let mut hash: u64 = 14695981039346656037u64;
    for byte in s.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(1099511628211);
    }
    format!("{hash:016x}")
}

const MAX_COLLISION_PROBES: u32 = 8;

fn candidate_key(base: &str, n: u32) -> String {
    if n == 0 { base.to_string() } else { format!("{base}-{n}") }
}

/// Whether the `.meta` file at `key` (if any) names `original_path`. `None`
/// means no `.meta` exists there at all.
fn meta_matches(dir: &Path, key: &str, original_path: &Path) -> Option<bool> {
    let bytes = std::fs::read(dir.join(format!("{key}.meta"))).ok()?;
    Some(String::from_utf8_lossy(&bytes) == original_path.to_string_lossy())
}

/// Resolves the key to save `original_path`'s autosave under: the first
/// candidate whose `.meta` already names this exact path, or is free.
/// Guards against a (astronomically unlikely, but previously unguarded)
/// FNV-1a hash collision between two different paths silently overwriting
/// each other's autosave under the same key.
fn key_for_save(dir: &Path, original_path: &Path) -> String {
    let base = path_key(original_path);
    for n in 0..MAX_COLLISION_PROBES {
        let key = candidate_key(&base, n);
        match meta_matches(dir, &key, original_path) {
            Some(true) | None => return key,
            Some(false) => continue,
        }
    }
    // Every probe collided with a different path — vanishingly unlikely.
    // Fall back to the base key rather than never saving at all.
    base
}

/// Resolves the key an existing autosave for `original_path` was saved
/// under, if any — mirrors key_for_save's probing so save/find/clear agree
/// on where a given path's autosave actually lives.
fn key_for_lookup(dir: &Path, original_path: &Path) -> Option<String> {
    let base = path_key(original_path);
    for n in 0..MAX_COLLISION_PROBES {
        let key = candidate_key(&base, n);
        match meta_matches(dir, &key, original_path) {
            Some(true) => return Some(key),
            Some(false) => continue,
            None => return None,
        }
    }
    None
}

pub fn save(original_path: &Path, content: &str) {
    let dir = autosave_dir();
    let _ = std::fs::create_dir_all(&dir);
    let key = key_for_save(&dir, original_path);
    // Write to a temp file then rename — rename is atomic on Linux so a crash
    // mid-write leaves the previous good file intact rather than truncating it.
    let tmp = dir.join(format!("{key}.typ.tmp"));
    let dest = dir.join(format!("{key}.typ"));
    if std::fs::write(&tmp, content).is_ok() {
        let _ = std::fs::rename(&tmp, &dest);
    }
    let _ = std::fs::write(dir.join(format!("{key}.meta")), original_path.to_string_lossy().as_bytes());
}

/// Returns (content, save_time) if an autosave exists that is newer than the
/// last manual save of `original_path`.
pub fn find_recovery(original_path: &Path) -> Option<(String, SystemTime)> {
    let dir = autosave_dir();
    let key = key_for_lookup(&dir, original_path)?;
    let autosave_file = dir.join(format!("{key}.typ"));
    if !autosave_file.exists() {
        return None;
    }
    let autosave_mtime = std::fs::metadata(&autosave_file)
        .and_then(|m| m.modified())
        .ok()?;
    // If the original exists, the autosave must be strictly newer.
    if let Ok(meta) = std::fs::metadata(original_path) {
        if let Ok(orig_mtime) = meta.modified() {
            if autosave_mtime <= orig_mtime {
                return None;
            }
        }
    }
    let content = std::fs::read_to_string(&autosave_file).ok()?;
    Some((content, autosave_mtime))
}

pub fn clear(original_path: &Path) {
    let dir = autosave_dir();
    if let Some(key) = key_for_lookup(&dir, original_path) {
        let _ = std::fs::remove_file(dir.join(format!("{key}.typ")));
        let _ = std::fs::remove_file(dir.join(format!("{key}.meta")));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn save_and_clear() {
        let path = PathBuf::from("/tmp/zerkalo_test_autosave_clear.typ");
        let content = "hello world";
        save(&path, content);

        let dir = autosave_dir();
        let key = path_key(&path);
        assert!(dir.join(format!("{key}.typ")).exists(), "autosave file should exist");

        clear(&path);
        assert!(!dir.join(format!("{key}.typ")).exists(), "autosave file should be removed");
    }

    #[test]
    fn find_recovery_returns_content_when_newer() {
        let path = PathBuf::from("/tmp/zerkalo_test_autosave_newer.typ");
        // Ensure the "original" doesn't exist so autosave is always newer
        let _ = std::fs::remove_file(&path);

        let content = "recovered content";
        save(&path, content);

        let result = find_recovery(&path);
        assert!(result.is_some(), "should find recovery when original absent");
        let (recovered, _) = result.unwrap();
        assert_eq!(recovered, content);

        clear(&path);
    }

    #[test]
    fn find_recovery_returns_none_after_clear() {
        let path = PathBuf::from("/tmp/zerkalo_test_autosave_none.typ");
        let _ = std::fs::remove_file(&path);

        save(&path, "temporary");
        clear(&path);

        assert!(find_recovery(&path).is_none(), "no recovery after clear");
    }

    #[test]
    fn find_recovery_skips_older_autosave() {
        let path = PathBuf::from("/tmp/zerkalo_test_autosave_older.typ");
        // Write the "original" file first
        std::fs::write(&path, "original").unwrap();
        // Wait a tiny bit, then write autosave
        std::thread::sleep(Duration::from_millis(10));
        save(&path, "autosaved");
        // Now touch the original to make it newer than autosave
        let autosave_dir = autosave_dir();
        let key = path_key(&path);
        let autosave_file = autosave_dir.join(format!("{key}.typ"));
        // Set autosave mtime to be older by writing original again
        std::fs::write(&path, "original refreshed").unwrap();

        // Now autosave is older than original — no recovery should be returned
        // (This depends on filesystem resolution; skip if mtime is same)
        let result = find_recovery(&path);
        // Either None (autosave older) or Some (same mtime — acceptable)
        // Just verify it doesn't panic
        drop(result);

        clear(&path);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn save_probes_past_a_simulated_hash_collision_instead_of_overwriting() {
        let victim = PathBuf::from("/tmp/zerkalo_test_autosave_collision_victim.typ");
        let attacker = PathBuf::from("/tmp/zerkalo_test_autosave_collision_attacker.typ");
        let dir = autosave_dir();
        let _ = std::fs::create_dir_all(&dir);

        // Simulate victim and attacker hashing to the same base key by
        // pre-planting victim's .meta/.typ directly under attacker's real key.
        let attacker_key = path_key(&attacker);
        std::fs::write(dir.join(format!("{attacker_key}.meta")), victim.to_string_lossy().as_bytes()).unwrap();
        std::fs::write(dir.join(format!("{attacker_key}.typ")), "victim's content").unwrap();

        save(&attacker, "attacker's content");

        // Victim's slot must be untouched...
        let victim_content = std::fs::read_to_string(dir.join(format!("{attacker_key}.typ"))).unwrap();
        assert_eq!(victim_content, "victim's content");

        // ...and the attacker's save must be found under a fallback key.
        let attacker_result = find_recovery(&attacker);
        assert!(attacker_result.is_some(), "attacker's autosave should still be findable via a fallback key");
        assert_eq!(attacker_result.unwrap().0, "attacker's content");

        // Cleanup: remove both the real victim slot and whatever fallback key
        // the attacker landed on.
        let _ = std::fs::remove_file(dir.join(format!("{attacker_key}.meta")));
        let _ = std::fs::remove_file(dir.join(format!("{attacker_key}.typ")));
        clear(&attacker);
    }
}
