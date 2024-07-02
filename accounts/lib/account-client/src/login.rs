use accounts_base::client::{account_data::AccountDataForClient, login::login_email};

use crate::{
    interface::Io,
    login_token_email::LoginResult,
    safe_interface::{IoSafe, SafeIo},
};

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
    let (login_req, login_data) = login_email(login_token_b64).map_err(LoginResult::Other)?;

    io.request_login(login_req).await?;
    io.write_serialized_session_key_pair(&login_data).await?;

    Ok(login_data)
}
