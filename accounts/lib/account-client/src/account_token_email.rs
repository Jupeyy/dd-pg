use accounts_shared::client::account_token::AccountTokenEmailRequest;

use thiserror::Error;

use crate::{
    errors::{FsLikeError, HttpLikeError},
    interface::Io,
    safe_interface::{IoSafe, SafeIo},
};

/// The result of a [`account`] request.
#[derive(Error, Debug)]
pub enum AccountResult {
    /// Crypt functions of main secret to be readable with the account
    /// server secret failed.
    /// If this occurss it is best to suggest the user to do a password
    /// reset, this looks unrecoverable (and should not really happen).
    #[error("Crypting related functions related to the main secret for the session failed: {0}")]
    MainSecretCryptFailed(anyhow::Error),
    /// A http like error occurred.
    #[error("{0}")]
    HttpLikeError(HttpLikeError),
    /// A fs like error occurred.
    #[error("{0}")]
    FsLikeError(FsLikeError),
    /// Errors that are not handled explicitly.
    #[error("Account failed: {0}")]
    Other(anyhow::Error),
}

impl From<HttpLikeError> for AccountResult {
    fn from(value: HttpLikeError) -> Self {
        Self::HttpLikeError(value)
    }
}

impl From<FsLikeError> for AccountResult {
    fn from(value: FsLikeError) -> Self {
        Self::FsLikeError(value)
    }
}

/// Generate a token sent by email for a new session/account.
pub async fn account_token_email(
    email: email_address::EmailAddress,
    io: &dyn Io,
) -> anyhow::Result<(), AccountResult> {
    account_token_email_impl(email, io.into()).await
}

async fn account_token_email_impl(
    email: email_address::EmailAddress,
    io: IoSafe<'_>,
) -> anyhow::Result<(), AccountResult> {
    io.request_account_token_email(AccountTokenEmailRequest { email })
        .await?;

    Ok(())
}
