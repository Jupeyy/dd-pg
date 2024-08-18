use accounts_shared::client::login_token_email::LoginTokenEmailRequest;

use thiserror::Error;

use crate::{
    errors::{FsLikeError, HttpLikeError},
    interface::Io,
    safe_interface::{IoSafe, SafeIo},
};

/// The result of a [`login_token_email`] request.
#[derive(Error, Debug)]
pub enum LoginTokenResult {
    /// A http like error occurred.
    #[error("{0}")]
    HttpLikeError(HttpLikeError),
    /// A fs like error occurred.
    #[error("{0}")]
    FsLikeError(FsLikeError),
    /// Errors that are not handled explicitly.
    #[error("Login failed: {0}")]
    Other(anyhow::Error),
}

impl From<HttpLikeError> for LoginTokenResult {
    fn from(value: HttpLikeError) -> Self {
        Self::HttpLikeError(value)
    }
}

impl From<FsLikeError> for LoginTokenResult {
    fn from(value: FsLikeError) -> Self {
        Self::FsLikeError(value)
    }
}

/// Generate a token sent by email for a new session/account.
pub async fn login_token_email(
    email: email_address::EmailAddress,
    io: &dyn Io,
) -> anyhow::Result<(), LoginTokenResult> {
    login_token_email_impl(email, io.into()).await
}

async fn login_token_email_impl(
    email: email_address::EmailAddress,
    io: IoSafe<'_>,
) -> anyhow::Result<(), LoginTokenResult> {
    io.request_login_email_token(LoginTokenEmailRequest { email })
        .await?;

    Ok(())
}
