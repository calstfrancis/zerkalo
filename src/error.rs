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
