use thiserror::Error;

pub type Result<T> = std::result::Result<T, GlorpError>;

#[derive(Debug, Error)]
pub enum GlorpError {
    #[error("{0}")]
    Message(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Sql(#[from] rusqlite::Error),
}
