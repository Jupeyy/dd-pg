use serde::{Deserialize, Serialize};
use thiserror::Error;

/// An error related to validating if a
/// request is allowed on the account server.
#[derive(Error, Debug, Clone, Serialize, Deserialize)]
pub enum AccountServerRequestError<E> {
    /// The request failed because
    /// the client is rate limited.
    #[error("{0}")]
    RateLimited(String),
    /// Banned because of using a blocked VPN.
    #[error("{0}")]
    VpnBan(String),
    /// Any kind of layer reported to block this connection.
    #[error("{0}")]
    Other(String),
    /// Database errors or similar.
    #[error("{target}: {err}. Bt: {bt}")]
    Unexpected {
        /// Where the error happened
        target: String,
        /// The error as string
        err: String,
        /// A backtrace.
        bt: String,
    },
    /// Error caused by the logic.
    #[error("{0}")]
    LogicError(E),
}
