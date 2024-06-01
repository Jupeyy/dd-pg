use accounts_base::client::password_reset::PasswordResetRequest;

use crate::{
    interface::Io,
    register::{register_internal, RegisterError, RegisterSuccess},
    safe_interface::{IoSafe, SafeIo},
};

/// Ask the account server to reset the password
/// to the given password given a reset code.
pub async fn password_reset(
    email: email_address::EmailAddress,
    reset_code_base64: String,
    new_password: &str,
    io: &dyn Io,
) -> anyhow::Result<RegisterSuccess, RegisterError> {
    password_reset_impl(email, reset_code_base64, new_password, io.into()).await
}

async fn password_reset_impl(
    email: email_address::EmailAddress,
    reset_code_base64: String,
    new_password: &str,
    io: IoSafe<'_>,
) -> anyhow::Result<RegisterSuccess, RegisterError> {
    register_internal(email, new_password, &io, |register_data| {
        Box::pin(async {
            Ok(io
                .send_password_reset(PasswordResetRequest {
                    reset_code_base64,
                    register_data,
                })
                .await?
                .register_res)
        })
    })
    .await
}
