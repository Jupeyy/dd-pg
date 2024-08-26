use accounts_shared::{
    account_server::{errors::AccountServerRequestError, login::LoginError},
    client::{account_data::AccountDataForClient, login},
};
use thiserror::Error;

use crate::{
    errors::{FsLikeError, HttpLikeError},
    interface::Io,
    safe_interface::{IoSafe, SafeIo},
};

/// The result of a [`login`] request.
#[derive(Error, Debug)]
pub enum LoginResult {
    /// A http like error occurred.
    #[error("{0}")]
    HttpLikeError(HttpLikeError),
    /// A fs like error occurred.
    #[error("{0}")]
    FsLikeError(FsLikeError),
    /// The account server responded with an error.
    #[error("{0}")]
    AccountServerRequstError(AccountServerRequestError<LoginError>),
    /// Errors that are not handled explicitly.
    #[error("Login failed: {0}")]
    Other(anyhow::Error),
}

impl From<HttpLikeError> for LoginResult {
    fn from(value: HttpLikeError) -> Self {
        Self::HttpLikeError(value)
    }
}

impl From<FsLikeError> for LoginResult {
    fn from(value: FsLikeError) -> Self {
        Self::FsLikeError(value)
    }
}

/// Create a new session (or account if not existed) on the account server.
pub async fn login(
    login_token_b64: String,
    io: &dyn Io,
) -> anyhow::Result<AccountDataForClient, LoginResult> {
    login_impl(login_token_b64, io.into()).await
}

async fn login_impl(
    login_token_b64: String,
    io: IoSafe<'_>,
) -> anyhow::Result<AccountDataForClient, LoginResult> {
    let (login_req, login_data) = login::login(login_token_b64).map_err(LoginResult::Other)?;

    io.request_login(login_req)
        .await?
        .map_err(LoginResult::AccountServerRequstError)?;
    io.write_serialized_session_key_pair(&login_data).await?;

    Ok(login_data)
}
