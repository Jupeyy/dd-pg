use accounts_shared::client::delete;
use thiserror::Error;

use crate::{
    errors::{FsLikeError, HttpLikeError},
    interface::Io,
    safe_interface::{IoSafe, SafeIo},
};

/// The result of a [`delete`] request.
#[derive(Error, Debug)]
pub enum DeleteResult {
    /// A http like error occurred.
    #[error("{0}")]
    HttpLikeError(HttpLikeError),
    /// A fs like error occurred.
    #[error("{0}")]
    FsLikeError(FsLikeError),
    /// Errors that are not handled explicitly.
    #[error("Delete failed: {0}")]
    Other(anyhow::Error),
}

impl From<HttpLikeError> for DeleteResult {
    fn from(value: HttpLikeError) -> Self {
        Self::HttpLikeError(value)
    }
}

impl From<FsLikeError> for DeleteResult {
    fn from(value: FsLikeError) -> Self {
        Self::FsLikeError(value)
    }
}

/// Delete an account on the account server.
pub async fn delete(account_token_b64: String, io: &dyn Io) -> anyhow::Result<(), DeleteResult> {
    delete_impl(account_token_b64, io.into()).await
}

async fn delete_impl(
    account_token_b64: String,
    io: IoSafe<'_>,
) -> anyhow::Result<(), DeleteResult> {
    let delete_req = delete::delete(account_token_b64).map_err(DeleteResult::Other)?;

    io.request_delete_account(delete_req).await?;
    // this is generally allowed to fail
    let _ = io.remove_serialized_session_key_pair().await;

    Ok(())
}

/// The result of a [`delete_sessions`] request.
#[derive(Error, Debug)]
pub enum DeleteSessionsResult {
    /// A http like error occurred.
    #[error("{0}")]
    HttpLikeError(HttpLikeError),
    /// A fs like error occurred.
    #[error("{0}")]
    FsLikeError(FsLikeError),
    /// Errors that are not handled explicitly.
    #[error("Delete failed: {0}")]
    Other(anyhow::Error),
}

impl From<HttpLikeError> for DeleteSessionsResult {
    fn from(value: HttpLikeError) -> Self {
        Self::HttpLikeError(value)
    }
}

impl From<FsLikeError> for DeleteSessionsResult {
    fn from(value: FsLikeError) -> Self {
        Self::FsLikeError(value)
    }
}

/// Delete all sessions of an account on the account server.
pub async fn delete_sessions(
    account_token_b64: String,
    io: &dyn Io,
) -> anyhow::Result<(), DeleteSessionsResult> {
    delete_session_impl(account_token_b64, io.into()).await
}

async fn delete_session_impl(
    account_token_b64: String,
    io: IoSafe<'_>,
) -> anyhow::Result<(), DeleteSessionsResult> {
    let delete_req =
        delete::delete_sessions(account_token_b64).map_err(DeleteSessionsResult::Other)?;

    io.request_delete_sessions(delete_req).await?;
    // this is generally allowed to fail
    let _ = io.remove_serialized_session_key_pair().await;

    Ok(())
}
