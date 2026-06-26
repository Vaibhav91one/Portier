use thiserror::Error;

pub type Result<T> = std::result::Result<T, PortierError>;

#[derive(Error, Debug)]
pub enum PortierError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON serialization error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("YAML serialization error: {0}")]
    Yaml(#[from] serde_yaml::Error),
    #[error("Port conflict: {0}")]
    Conflict(String),
    #[error("Not found: {0}")]
    NotFound(String),
    #[error("Registry access error: {0}")]
    Registry(String),
    #[error("Config error: {0}")]
    Config(String),
    #[error("Invalid input: {0}")]
    InvalidInput(String),
    #[error("{0}")]
    Other(String),
}
