#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("{0}")]
    Invalid(String),
    /// A rejected move (`MoveError`): the HTTP layer answers 400 with this message.
    #[error("{0}")]
    Move(String),
    #[error("database error: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    /// A wiki request failed (network, bad status, unreadable response).
    #[error("wiki error: {0}")]
    Wiki(String),
}

pub type Result<T> = std::result::Result<T, CoreError>;
