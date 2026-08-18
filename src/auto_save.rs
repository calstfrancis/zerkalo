use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// Recovery copies are regenerable state, not settings, so they belong under
/// the XDG state dir rather than in `~/.config` alongside `config.toml`.
/// `legacy_autosave_dir` is still read once, at `prune()`, to move anything
/// left over from the old location.
fn autosave_dir() -> PathBuf {
    if let Some(dir) = TEST_DIR.with(|d| d.borrow().clone()) {
        return dir;
    }
    let base = std::env::var("XDG_STATE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(shellexpand::tilde("~/.local/state").into_owned()));
    base.join("zerkalo/autosave")
}

fn legacy_autosave_dir() -> PathBuf {
    PathBuf::from(shellexpand::tilde("~/.config/zerkalo/autosave").into_owned())
}

thread_local! {
    /// Redirects the autosave directory for tests. Without it the unit tests
    /// read and write the user's real recovery files and race each other under
    /// the default parallel test runner.
    static TEST_DIR: std::cell::RefCell<Option<PathBuf>> = const { std::cell::RefCell::new(None) };
}

/// Autosaves are dropped once they are this old. A recovery copy is only ever
/// interesting until the user next opens that file; anything this stale is for
/// a document they are not coming back to.
const MAX_AGE: Duration = Duration::from_secs(30 * 24 * 60 * 60);

/// Housekeeping at startup: migrate anything left in the old config-dir
/// location, then drop entries whose original file is gone or that have aged
/// out. Nothing here ever removed entries before, so the directory grew for the
/// life of the install, including for files deleted long ago.
pub fn prune() {
    let dir = autosave_dir();
    let _ = std::fs::create_dir_all(&dir);

    let legacy = legacy_autosave_dir();
    if legacy != dir && legacy.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&legacy) {
            for entry in entries.flatten() {
                let dest = dir.join(entry.file_name());
                if !dest.exists() {
                    let _ = std::fs::rename(entry.path(), &dest);
                }
            }
        }
        let _ = std::fs::remove_dir(&legacy);
    }

    let Ok(entries) = std::fs::read_dir(&dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_none_or(|e| e != "meta") {
            continue;
        }
        let Some(key) = path.file_stem().map(|s| s.to_string_lossy().into_owned()) else {
            continue;
        };
        let doc = dir.join(format!("{key}.typ"));

        let aged_out = std::fs::metadata(&doc)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| SystemTime::now().duration_since(t).ok())
            .is_some_and(|age| age > MAX_AGE);

        // Deliberately not pruning entries whose original file has gone: if the
        // document was deleted, this copy is the only one left. Age is the only
        // thing that retires a recovery copy.
        if aged_out || !doc.exists() {
            let _ = std::fs::remove_file(&path);
            let _ = std::fs::remove_file(&doc);
        }
    }
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
    if n == 0 {
        base.to_string()
    } else {
        format!("{base}-{n}")
    }
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

/// Returns `true` if the recovery copy was actually written. Callers that
/// tell the user "Autosaved" must check this rather than assume success.
pub fn save(original_path: &Path, content: &str) -> bool {
    let dir = autosave_dir();
    if std::fs::create_dir_all(&dir).is_err() {
        return false;
    }
    let key = key_for_save(&dir, original_path);
    let dest = dir.join(format!("{key}.typ"));
    let wrote_content = crate::error::atomic_write(&dest, content.as_bytes()).is_ok();
    let wrote_meta = crate::error::atomic_write(
        &dir.join(format!("{key}.meta")),
        original_path.to_string_lossy().as_bytes(),
    )
    .is_ok();
    wrote_content && wrote_meta
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

    /// Redirects `autosave_dir()` at a throwaway directory for the duration of
    /// one test. These tests used to read and write the user's real recovery
    /// files under `~/.config/zerkalo/autosave`, which both polluted live data
    /// and made them race each other under the parallel test runner.
    struct Sandbox {
        dir: tempfile::TempDir,
    }

    impl Sandbox {
        fn new() -> Self {
            let dir = tempfile::tempdir().unwrap();
            TEST_DIR.with(|d| *d.borrow_mut() = Some(dir.path().to_path_buf()));
            Self { dir }
        }
        fn path(&self) -> &Path {
            self.dir.path()
        }
        fn doc(&self, name: &str) -> PathBuf {
            self.dir.path().join(name)
        }
    }

    impl Drop for Sandbox {
        fn drop(&mut self) {
            TEST_DIR.with(|d| *d.borrow_mut() = None);
        }
    }

    #[test]
    fn save_and_clear() {
        let sb = Sandbox::new();
        let path = sb.doc("clear.typ");
        assert!(
            save(&path, "hello world"),
            "save should report success on a writable dir"
        );

        let key = path_key(&path);
        assert!(
            sb.path().join(format!("{key}.typ")).exists(),
            "autosave file should exist"
        );

        clear(&path);
        assert!(
            !sb.path().join(format!("{key}.typ")).exists(),
            "autosave file should be removed"
        );
    }

    #[test]
    fn save_reports_failure_instead_of_lying_when_the_write_fails() {
        use std::os::unix::fs::PermissionsExt;
        let sb = Sandbox::new();
        let path = sb.doc("readonly.typ");

        let original_mode = std::fs::metadata(sb.path()).unwrap().permissions().mode();
        std::fs::set_permissions(sb.path(), std::fs::Permissions::from_mode(0o500)).unwrap();
        let ok = save(&path, "should not land");
        // Restore before any assert can early-return, so the sandbox's TempDir
        // can still clean itself up on drop.
        std::fs::set_permissions(sb.path(), std::fs::Permissions::from_mode(original_mode))
            .unwrap();

        assert!(
            !ok,
            "save must report failure when the autosave dir isn't writable, not pretend success"
        );
    }

    #[test]
    fn find_recovery_returns_content_when_newer() {
        let sb = Sandbox::new();
        // The "original" never exists, so the autosave is always newer.
        let path = sb.doc("newer.typ");
        save(&path, "recovered content");

        let result = find_recovery(&path);
        assert!(
            result.is_some(),
            "should find recovery when original absent"
        );
        assert_eq!(result.unwrap().0, "recovered content");
    }

    #[test]
    fn find_recovery_returns_none_after_clear() {
        let sb = Sandbox::new();
        let path = sb.doc("none.typ");
        save(&path, "temporary");
        clear(&path);

        assert!(find_recovery(&path).is_none(), "no recovery after clear");
    }

    #[test]
    fn find_recovery_skips_an_autosave_older_than_the_file() {
        let sb = Sandbox::new();
        let path = sb.doc("older.typ");
        save(&path, "autosaved");

        // Write the original with an mtime strictly after the autosave's.
        let key = path_key(&path);
        let autosave_mtime = std::fs::metadata(sb.path().join(format!("{key}.typ")))
            .unwrap()
            .modified()
            .unwrap();
        std::fs::write(&path, "original refreshed").unwrap();
        let later = autosave_mtime + Duration::from_secs(10);
        set_mtime(&path, later);

        assert!(
            find_recovery(&path).is_none(),
            "an autosave older than the file on disk is not a recovery"
        );
    }

    #[test]
    fn save_probes_past_a_simulated_hash_collision_instead_of_overwriting() {
        let sb = Sandbox::new();
        let victim = sb.doc("collision-victim.typ");
        let attacker = sb.doc("collision-attacker.typ");

        // Simulate victim and attacker hashing to the same base key by
        // pre-planting victim's .meta/.typ directly under attacker's real key.
        let attacker_key = path_key(&attacker);
        std::fs::write(
            sb.path().join(format!("{attacker_key}.meta")),
            victim.to_string_lossy().as_bytes(),
        )
        .unwrap();
        std::fs::write(
            sb.path().join(format!("{attacker_key}.typ")),
            "victim's content",
        )
        .unwrap();

        save(&attacker, "attacker's content");

        let victim_content =
            std::fs::read_to_string(sb.path().join(format!("{attacker_key}.typ"))).unwrap();
        assert_eq!(
            victim_content, "victim's content",
            "victim's slot must be untouched"
        );

        let attacker_result = find_recovery(&attacker);
        assert!(
            attacker_result.is_some(),
            "attacker's autosave should still be findable via a fallback key"
        );
        assert_eq!(attacker_result.unwrap().0, "attacker's content");
    }

    #[test]
    fn prune_drops_aged_out_entries_and_keeps_fresh_ones() {
        let sb = Sandbox::new();
        let fresh = sb.doc("fresh.typ");
        let stale = sb.doc("stale.typ");
        save(&fresh, "keep me");
        save(&stale, "let me go");

        let stale_key = path_key(&stale);
        let long_ago = SystemTime::now() - MAX_AGE - Duration::from_secs(60);
        set_mtime(&sb.path().join(format!("{stale_key}.typ")), long_ago);

        prune();

        assert!(
            find_recovery(&fresh).is_some(),
            "a recent autosave must survive pruning"
        );
        assert!(
            find_recovery(&stale).is_none(),
            "an aged-out autosave should be removed"
        );
    }

    #[test]
    fn prune_removes_a_meta_left_without_its_document() {
        let sb = Sandbox::new();
        let path = sb.doc("halfpair.typ");
        save(&path, "content");
        let key = path_key(&path);
        std::fs::remove_file(sb.path().join(format!("{key}.typ"))).unwrap();

        prune();

        assert!(
            !sb.path().join(format!("{key}.meta")).exists(),
            "a .meta with no .typ is a broken pair and should not linger"
        );
    }

    fn set_mtime(path: &Path, when: SystemTime) {
        let f = std::fs::OpenOptions::new().write(true).open(path).unwrap();
        f.set_modified(when).unwrap();
    }
}
