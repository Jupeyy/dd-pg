use accounts_base::{
    account_server::register_token::RegisterToken,
    client::{
        auth::prepare_auth_request, otp::OtpRequest, reigster_token::RegisterTokenRequest,
        session::SessionDataForClient,
    },
};
use anyhow::anyhow;

use crate::{
    interface::Io,
    machine_id::machine_uid,
    safe_interface::{IoSafe, SafeIo},
};

/// Requests a register token from the account server
/// that can be send to the game server to verify the account
/// id of the client.  
/// Note that the packet sent to the game server should always be
/// signed using the client's private key.
pub async fn request_register_token_from_account_server(
    io: &dyn Io,
) -> anyhow::Result<RegisterToken> {
    request_register_token_from_account_server_impl(io.into()).await
}

async fn request_register_token_from_account_server_impl(
    io: IoSafe<'_>,
) -> anyhow::Result<RegisterToken> {
    // generate one time password for this request
    let otp_res = io.request_otp(OtpRequest { count: 1 }).await?;

    let otps = otp_res
        .otps
        .try_into()
        .map_err(|_| anyhow!("Invalid otp response. Expected 1 one time password."))?;
    let [otp] = otps;

    // read session's key-pair
    let SessionDataForClient {
        mut priv_key,
        pub_key,
    } = io.read_session_key_pair_file().await?;

    let hashed_hw_id = machine_uid()?;

    let register_token = io
        .request_register_token(RegisterTokenRequest {
            auth_req: prepare_auth_request(otp, hashed_hw_id, &mut priv_key, pub_key),
        })
        .await?;

    Ok(register_token.token)
}
