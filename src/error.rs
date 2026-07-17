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
pub fn atomic_write(path: &std::path::Path, contents: &[u8]) -> std::io::Result<()> {
    let mut tmp_name = path.file_name().unwrap_or_default().to_os_string();
    tmp_name.push(".tmp");
    let tmp = path.with_file_name(tmp_name);
    std::fs::write(&tmp, contents)?;
    let result = std::fs::rename(&tmp, path);
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}
