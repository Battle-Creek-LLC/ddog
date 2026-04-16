use thiserror::Error;

pub type Result<T> = std::result::Result<T, ApiError>;

#[derive(Debug, Error)]
pub enum ApiError {
    #[error("authentication failed (401/403): check DD_API_KEY and DD_APP_KEY")]
    Auth,

    #[error("not found (404): {0}")]
    NotFound(String),

    #[error("rate limited (429); retry after {retry_after_secs}s")]
    RateLimited { retry_after_secs: u64 },

    #[error("upstream error {status}: {body}")]
    Upstream { status: u16, body: String },

    #[error("network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("url error: {0}")]
    Url(#[from] url::ParseError),

    #[error("decode error: {0}")]
    Decode(#[from] serde_json::Error),
}

impl ApiError {
    /// Exit code mapping matching the SPECIFICATION.
    pub fn exit_code(&self) -> i32 {
        match self {
            ApiError::Auth => 2,
            ApiError::NotFound(_) => 3,
            ApiError::RateLimited { .. } => 4,
            ApiError::Upstream { .. } => 5,
            ApiError::Network(_) => 6,
            _ => 1,
        }
    }
}
