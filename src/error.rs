use thiserror::Error;

#[derive(Debug, Error)]
pub enum ZerkaloError {
    #[error("IO: {0}")]
    Io(#[from] std::io::Error),
    #[error("Git: {0}")]
    Git(#[from] git2::Error),
    #[error("Config parse: {0}")]
    ConfigParse(#[from] toml::de::Error),
    #[error("Config serialize: {0}")]
    ConfigSerialize(#[from] toml::ser::Error),
    #[allow(dead_code)]
    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, ZerkaloError>;

/// Writes `contents` to `path` via a temp-file-then-rename so a crash or
/// power loss mid-write leaves the previous good file intact rather than
/// truncating it (rename is atomic on Linux).
///
/// The temp file is fsynced before the rename: rename is atomic with respect to
/// *ordering*, but without the fsync a power loss can land the rename while the
/// data blocks are still in the page cache, leaving a correctly-named file full
/// of zeroes. This is the path every document save takes, so the guarantee has
/// to be the real one.
///
/// The temp name includes the process id and a counter so two writers (or a
/// retry after a failure) can't collide on one another's temp file.
pub fn atomic_write(path: &std::path::Path, contents: &[u8]) -> std::io::Result<()> {
    use std::io::Write;

    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

    let mut tmp_name = path.file_name().unwrap_or_default().to_os_string();
    tmp_name.push(format!(".{}.{n}.tmp", std::process::id()));
    let tmp = path.with_file_name(tmp_name);

    let write_and_sync = || -> std::io::Result<()> {
        let mut file = std::fs::File::create(&tmp)?;
        // Keep the original's permissions; File::create would otherwise reset a
        // deliberately-restricted file to the default 0644 on every save.
        if let Ok(meta) = std::fs::metadata(path) {
            let _ = file.set_permissions(meta.permissions());
        }
        file.write_all(contents)?;
        file.sync_all()
    };

    if let Err(e) = write_and_sync() {
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }

    let result = std::fs::rename(&tmp, path);
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
        return result;
    }

    // Durably record the rename itself, so the new name survives a crash too.
    if let Some(dir) = path.parent() {
        if let Ok(handle) = std::fs::File::open(dir) {
            let _ = handle.sync_all();
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atomic_write_replaces_content_and_leaves_no_temp_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("doc.typ");
        std::fs::write(&path, "original").unwrap();

        atomic_write(&path, b"replacement").unwrap();

        assert_eq!(std::fs::read_to_string(&path).unwrap(), "replacement");
        let strays: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.ends_with(".tmp"))
            .collect();
        assert!(strays.is_empty(), "temp files left behind: {strays:?}");
    }

    #[cfg(unix)]
    #[test]
    fn atomic_write_preserves_permissions() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("private.typ");
        std::fs::write(&path, "secret").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();

        atomic_write(&path, b"still secret").unwrap();

        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(
            mode, 0o600,
            "a 0600 document must not become world-readable on save"
        );
    }

    #[test]
    fn atomic_write_creates_a_file_that_does_not_exist_yet() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("new.typ");
        atomic_write(&path, b"fresh").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "fresh");
    }
}
