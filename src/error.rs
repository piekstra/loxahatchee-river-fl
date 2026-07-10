use std::fmt;

/// Application errors mapped to a stable exit-code contract (see README).
#[derive(Debug)]
pub enum AppError {
    /// Bad usage / missing or malformed input (e.g. an unparseable account id).
    Usage(String),
    /// Authentication required or failed (login needed, bad credentials).
    Auth(String),
    /// OS keychain failure.
    Keychain(String),
    /// No matching account / empty result set.
    NotFound(String),
    /// Network or upstream (non-2xx) failure.
    Network(String),
    /// HTTP 429 from the portal API.
    RateLimited,
    /// An async server-side request never completed within the poll budget.
    Timeout(String),
    /// Anything else.
    Other(String),
}

impl AppError {
    pub fn exit_code(&self) -> i32 {
        match self {
            AppError::Other(_) | AppError::Keychain(_) => 1,
            AppError::Usage(_) => 2,
            AppError::Auth(_) => 3,
            AppError::NotFound(_) => 4,
            AppError::Network(_) => 5,
            AppError::RateLimited => 6,
            AppError::Timeout(_) => 7,
        }
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::Usage(m) => write!(f, "{m}"),
            AppError::Auth(m) => write!(f, "{m}"),
            AppError::Keychain(m) => write!(f, "keychain error: {m}"),
            AppError::NotFound(m) => write!(f, "not found: {m}"),
            AppError::Network(m) => write!(f, "network/upstream error: {m}"),
            AppError::RateLimited => {
                write!(
                    f,
                    "rate limited by the portal (HTTP 429) — slow down and retry"
                )
            }
            AppError::Timeout(m) => write!(f, "timed out waiting for the server: {m}"),
            AppError::Other(m) => write!(f, "{m}"),
        }
    }
}

impl std::error::Error for AppError {}

impl From<reqwest::Error> for AppError {
    fn from(e: reqwest::Error) -> Self {
        AppError::Network(e.to_string())
    }
}
