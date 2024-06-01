use accounts_base::{
    account_server::{
        account_id::AccountId,
        auth::{AuthResponse, AuthResponseSuccess},
    },
    client::{
        auth::{decrypt_main_secret_from_auth_request, prepare_auth_request},
        otp::OtpRequest,
        session::SessionDataForClient,
    },
};
use anyhow::anyhow;
use thiserror::Error;

use crate::{
    errors::{FsLikeError, HttpLikeError},
    interface::Io,
    machine_id::machine_uid,
    safe_interface::{IoSafe, SafeIo},
};

/// The result of a [`login`] request.
#[derive(Error, Debug)]
pub enum AuthResult {
    /// Account verification required. User should check email
    /// or request a new verify mail.
    #[error("The account was not yet verified.")]
    AccountNotVerified,
    /// Session was invalid, must login again.
    #[error("The session was not valid anymore.")]
    SessionWasInvalid,
    /// Auth main secret was not able to decrypt main secret.
    /// It's best to do another login now.
    #[error("Decryption of session's main secret failed: {0}")]
    MainSecretCryptFailed(anyhow::Error),
    /// A file system like error occurred.
    /// This usually means the user was not yet logged in.
    #[error("{0}")]
    FsLikeError(FsLikeError),
    /// A http like error occurred.
    #[error("{0}")]
    HttpLikeError(HttpLikeError),
    /// Errors that are not handled explicitly.
    #[error("Auth failed: {0}")]
    Other(anyhow::Error),
}

impl From<HttpLikeError> for AuthResult {
    fn from(value: HttpLikeError) -> Self {
        Self::HttpLikeError(value)
    }
}

impl From<FsLikeError> for AuthResult {
    fn from(value: FsLikeError) -> Self {
        Self::FsLikeError(value)
    }
}

/// The auth data is used on the client
/// to decrypt key-pairs used
/// to verify your account on game server groups.
#[derive(Debug)]
pub struct AuthClientData {
    /// The main secret to decrypt game server group
    /// key-pairs.  
    /// __NEVER__ share or save this secret unencrypted anywhere.
    pub main_secret: Vec<u8>,
    /// This is the account id, it can be used to register
    /// to game servers permanently.
    pub account_id: AccountId,
}

/// Auth an existing session on the account server.
/// The account server will respond with a secret,
/// that can be used to decrypt the main secret
/// used for accounts on the client.  
/// __IMPORTANT__: The main secret should __NEVER__ be shared,
/// saved or otherwise leave memory. Instead do a new auth
/// every time the client opens.
///
/// # Errors
///
/// If an error occurs this usually means that the session is not valid anymore.
pub async fn auth(io: &dyn Io) -> anyhow::Result<AuthClientData, AuthResult> {
    auth_impl(io.into()).await
}

async fn auth_impl(io: IoSafe<'_>) -> anyhow::Result<AuthClientData, AuthResult> {
    // generate one time password for this request
    let mut otp_res = io.request_otp(OtpRequest { count: 1 }).await?;
    (!otp_res.otps.is_empty()).then_some(()).ok_or_else(|| {
        AuthResult::Other(anyhow!("Expected at least 1 one time password, got none"))
    })?;
    let otp = otp_res.otps.remove(0);

    // read session's key-pair
    let SessionDataForClient {
        mut priv_key,
        pub_key,
    } = io.read_session_key_pair_file().await?;

    let hashed_hw_id = machine_uid().map_err(AuthResult::Other)?;

    // do the auth request using the above private key
    let msg = prepare_auth_request(otp, hashed_hw_id, &mut priv_key, pub_key);
    let auth_res = io.send_auth(msg).await?;
    let auth_res = match auth_res {
        AuthResponse::Success(auth_res) => match auth_res {
            AuthResponseSuccess::Verified(auth_res) => auth_res,
            AuthResponseSuccess::NotVerified(_) => {
                return Err(AuthResult::AccountNotVerified);
            }
        },
        AuthResponse::Invalid => {
            return Err(AuthResult::SessionWasInvalid);
        }
    };

    // read the main secret that was encrypted using the secret from the account server
    let main_secret = io.read_server_encrypted_main_secret_file().await?;
    let main_secret = decrypt_main_secret_from_auth_request(main_secret, auth_res.secret)
        .map_err(AuthResult::MainSecretCryptFailed)?;

    Ok(AuthClientData {
        main_secret,
        account_id: auth_res.account_id,
    })
}
