use accounts_base::{
    account_server::{
        account_id::INVALID_ACCOUNT_ID, auth::AuthResponseSuccess, login::LoginResponse,
    },
    client::{
        account_data::decrypt_main_secret_from_password,
        auth::decrypt_main_secret_from_auth_request,
        otp::OtpRequest,
        session::{generate_session_data, reencrypt_main_secret_with_server_secret},
    },
};
use anyhow::anyhow;
use thiserror::Error;

use crate::{
    auth::AuthClientData,
    errors::HttpLikeError,
    interface::Io,
    machine_id::machine_uid,
    safe_interface::{IoSafe, SafeIo},
};

/// The result of the auth attempt during
/// a login.
#[derive(Debug)]
pub enum AuthLoginResult {
    /// The account is verified and can be used
    Verified(Box<AuthClientData>),
    /// The accout still needs verification
    NotVerified,
}

/// Contains all data that are a result of
/// the login process.
/// This includes an auth response, which
/// includes the main secret.
#[derive(Debug)]
pub struct LoginData {
    /// A login also automatically triggers an auth.
    pub auth: AuthLoginResult,
    /// If saving the keys/secrets failed,
    /// then this field contains the error for it.
    /// This however does not prevent the session to work
    /// for as long as the client does not forget
    /// about the session data.
    pub persist_res: anyhow::Result<()>,
}

/// The result of a [`login`] request.
#[derive(Error, Debug)]
pub enum LoginResult {
    /// The password or email was invalid.
    #[error("The password or login was incorrect")]
    InvalidPasswordOrEmail,
    /// Crypt functions of main secret to be readable with the account
    /// server secret failed.
    /// If this occurss it is best to suggest the user to do a password
    /// reset, this looks unrecoverable (and should not really happen).
    #[error("Crypting related functions related to the main secret for the session failed: {0}")]
    MainSecretCryptFailed(anyhow::Error),
    /// A http like error occurred.
    #[error("{0}")]
    HttpLikeError(HttpLikeError),
    /// Errors that are not handled explicitly.
    #[error("Login failed: {0}")]
    Other(anyhow::Error),
}

impl From<HttpLikeError> for LoginResult {
    fn from(value: HttpLikeError) -> Self {
        Self::HttpLikeError(value)
    }
}

/// Create a new session on the server
pub async fn login(
    email: email_address::EmailAddress,
    password: &str,
    io: &dyn Io,
) -> anyhow::Result<LoginData, LoginResult> {
    login_impl(email, password, io.into()).await
}

async fn login_impl(
    email: email_address::EmailAddress,
    password: &str,
    io: IoSafe<'_>,
) -> anyhow::Result<LoginData, LoginResult> {
    let otp_res = io.request_otp(OtpRequest { count: 2 }).await?;
    let otps = otp_res.otps.try_into().map_err(|_| {
        LoginResult::Other(anyhow!(
            "Invalid otp response. Expected 2 one time passwords."
        ))
    })?;

    let hashed_hw_id = machine_uid().map_err(LoginResult::Other)?;

    let [login_otp, auth_otp] = otps;
    let session_data = generate_session_data(login_otp, auth_otp, hashed_hw_id, email, password)
        .map_err(LoginResult::Other)?;

    let login_res = io.send_login(session_data.for_server).await?;

    match login_res {
        LoginResponse::Success(login_res) => {
            let main_secret_with_server_secret = reencrypt_main_secret_with_server_secret(
                login_res.main_secret,
                password,
                login_res.auth.secret().secret.clone(),
            )
            .map_err(LoginResult::MainSecretCryptFailed)?;

            let main_secret_with_server_secret_clone = main_secret_with_server_secret.clone();
            let main_secret = decrypt_main_secret_from_auth_request(
                main_secret_with_server_secret_clone,
                login_res.auth.secret().clone(),
            )
            .map_err(LoginResult::MainSecretCryptFailed)?;

            let persist_res = async {
                io.write_session_key_pair_file(session_data.for_client)
                    .await?;

                io.write_server_encrypted_main_secret_file(main_secret_with_server_secret)
                    .await?;

                anyhow::Ok(())
            };

            Ok(LoginData {
                auth: match login_res.auth {
                    AuthResponseSuccess::Verified(auth_res) => {
                        AuthLoginResult::Verified(Box::new(AuthClientData {
                            main_secret,
                            account_id: auth_res.account_id,
                        }))
                    }
                    AuthResponseSuccess::NotVerified(_) => AuthLoginResult::NotVerified,
                },
                persist_res: persist_res.await,
            })
        }
        LoginResponse::InvalidPasswordOrEmail => Err(LoginResult::InvalidPasswordOrEmail),
    }
}

/// Try to login offline using only the password,
/// the locally saved encrypted main secret
/// and the locally saved encrypted private keys for
/// the game server groups.
/// This can be useful if the account server is down,
/// or the user wants to play offline.
/// But is limited to secrets/keys that already exist on the client.
pub async fn login_offline(password: &str, io: &dyn Io) -> anyhow::Result<AuthClientData> {
    login_offline_impl(password, io.into()).await
}

async fn login_offline_impl(password: &str, io: IoSafe<'_>) -> anyhow::Result<AuthClientData> {
    let main_secret = io.read_encrypted_main_secret_file().await?;

    let main_secret = decrypt_main_secret_from_password(main_secret, password)?;

    Ok(AuthClientData {
        main_secret,
        // if the user plays offline
        // it should also not know about its account id
        // It cannot register to game servers anyway
        // And game servers should rely on the id generated
        // through the public key anyway
        account_id: INVALID_ACCOUNT_ID,
    })
}
