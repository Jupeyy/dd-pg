use accounts_base::client::password_forgot::PasswordForgotRequest;

use crate::{
    interface::Io,
    safe_interface::{IoSafe, SafeIo},
};

/// Ask the account server to trigger a password reset
/// process. If this function returns success the client
/// should tell the user to check its emails and
/// open a form to enter a reset token.
pub async fn password_forgot(
    email: email_address::EmailAddress,
    io: &dyn Io,
) -> anyhow::Result<()> {
    password_forgot_impl(email, io.into()).await
}

async fn password_forgot_impl(
    email: email_address::EmailAddress,
    io: IoSafe<'_>,
) -> anyhow::Result<()> {
    // Tell the account server about our plan to register
    io.request_password_forgot(PasswordForgotRequest { email })
        .await?;

    Ok(())
}
