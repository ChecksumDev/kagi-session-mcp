use thiserror::Error;

#[derive(Debug, Error)]
pub enum DomainError {
    #[error("no kagi session found in any installed browser")]
    NoSessionFound,

    #[error("session was found but kagi rejected it (likely expired or app-bound-encrypted): {0}")]
    SessionRejected(String),

    #[error("browser cookie store could not be read: {0}")]
    CookieStoreUnavailable(String),

    #[error(
        "could not decrypt cookie value (Chrome v20 App-Bound Encryption requires the user to provide a Session Link instead)"
    )]
    CookieDecryptionBlocked,

    #[error("kagi network request failed: {0}")]
    NetworkError(String),

    #[error("could not parse kagi response: {0}")]
    ParseError(String),

    #[error("invalid query: {0}")]
    InvalidQuery(String),
}

pub type DomainResult<T> = Result<T, DomainError>;
