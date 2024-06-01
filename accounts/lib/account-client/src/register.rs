use std::{future::Future, pin::Pin};

use accounts_base::{
    account_server::{auth::AuthResponse, register::RegisterResponse},
    client::{
        otp::OtpRequest,
        register::{register_data, RegisterDataForClient, RegisterDataForServer},
        session::reencrypt_main_secret_with_server_secret,
    },
};
use anyhow::anyhow;
use thiserror::Error;

use crate::{
    errors::HttpLikeError,
    interface::Io,
    machine_id::machine_uid,
    safe_interface::{IoSafe, SafeIo},
};

/// If the registration was complete an object
/// of this struct is returned, which contains
/// the next steps required (if any).
#[derive(Debug)]
pub struct RegisterSuccess {
    /// A verification is required
    /// to complete the account (e.g. by email).  
    /// Attemps to authing will fail before the account is verified.
    pub requires_verification: bool,
    /// The session was also created (no login required).  
    /// If this fails for whatever reason a new login is required.
    pub session_was_created: anyhow::Result<()>,
}

/// Errors related to a failed register attempt.
#[derive(Error, Debug)]
pub enum RegisterError {
    /// Password was too weak
    #[error(
        "
            password must be at least 8 characters long and must include at least one number,\n\
            capital & lowercase character and special symbol like `!@;` or similar
        "
    )]
    PasswordTooWeak,
    /// Account already exists with the given email
    #[error("An account with that email already exists")]
    AccountWithEmailAlreadyExists,
    /// A http like error occurred
    #[error("{0}")]
    HttpLikeError(HttpLikeError),
    /// Arbitrary error
    #[error("{0}")]
    Other(anyhow::Error),
}

impl From<anyhow::Error> for RegisterError {
    fn from(value: anyhow::Error) -> Self {
        Self::Other(value)
    }
}

impl From<HttpLikeError> for RegisterError {
    fn from(value: HttpLikeError) -> Self {
        Self::HttpLikeError(value)
    }
}

/// Full registering process for a client.
/// This is usually called when the user clicks register on a
/// register form.  
/// After a successful registration the client is automatically
/// logged in if [`RegisterSuccess::session_was_created`] is ok,
/// however that does not mean the client can do auth attempts yet.  
/// Note that after registering, if [`RegisterSuccess::requires_verification`]
/// returned `false` (so no verification is required), then
/// the client must do an [`crate::auth::auth`] attempt.
pub async fn register(
    email: email_address::EmailAddress,
    password: &str,
    io: &dyn Io,
) -> anyhow::Result<RegisterSuccess, RegisterError> {
    register_impl(email, password, io.into()).await
}

async fn register_impl(
    email: email_address::EmailAddress,
    password: &str,
    io: IoSafe<'_>,
) -> anyhow::Result<RegisterSuccess, RegisterError> {
    register_internal(email, password, &io, |register_data| {
        io.send_register(register_data)
    })
    .await
}

pub(crate) async fn handle_register_res(
    response: RegisterResponse,
    register_data: RegisterDataForClient,
    password: &str,
    io: &dyn Io,
) -> anyhow::Result<RegisterSuccess, RegisterError> {
    handle_register_res_impl(response, register_data, password, io.into()).await
}

async fn handle_register_res_impl(
    response: RegisterResponse,
    register_data: RegisterDataForClient,
    password: &str,
    io: IoSafe<'_>,
) -> anyhow::Result<RegisterSuccess, RegisterError> {
    match response {
        // In this moment the account is registered.
        RegisterResponse::Success {
            requires_verification,
            auth_response,
        } => {
            let session_was_created = match auth_response {
                AuthResponse::Success(auth_res) => {
                    // since registration was complete
                    // it is important that any `?` operator does not return
                    // from the register function
                    // if the session fails to be established (files to be written)
                    // that does not imply that the registration failed!
                    let write_login_data = async move {
                        io.write_encrypted_main_secret_file(
                            register_data.account_data.secret.clone(),
                        )
                        .await?;
                        io.write_session_key_pair_file(register_data.session_data)
                            .await?;

                        let main_secret_with_server_secret =
                            reencrypt_main_secret_with_server_secret(
                                register_data.account_data.secret,
                                password,
                                auth_res.secret().secret.clone(),
                            )?;

                        io.write_server_encrypted_main_secret_file(main_secret_with_server_secret)
                            .await?;
                        anyhow::Ok(())
                    };
                    write_login_data.await
                }
                AuthResponse::Invalid => Err(anyhow!("auth response was invalid")),
            };

            // Now the account server wants to establish a session
            Ok(RegisterSuccess {
                requires_verification,
                session_was_created,
            })
        }
        RegisterResponse::AccountWithEmailAlreadyExists => {
            Err(RegisterError::AccountWithEmailAlreadyExists)
        }
    }
}

pub(crate) fn satisfying_password_strength(password: &str) -> bool {
    password.len() >= 8
        && password.chars().any(|char| char.is_uppercase())
        && password.chars().any(|char| char.is_lowercase())
        && password.chars().any(|char| char.is_ascii_digit())
        && password.chars().any(|char| char.is_ascii_punctuation())
}

pub(crate) async fn register_internal<'a, F>(
    email: email_address::EmailAddress,
    password: &str,
    io: &IoSafe<'_>,
    send_func: F,
) -> anyhow::Result<RegisterSuccess, RegisterError>
where
    F: FnOnce(
            RegisterDataForServer,
        ) -> Pin<
            Box<dyn Future<Output = anyhow::Result<RegisterResponse, HttpLikeError>> + Send + 'a>,
        > + Send,
{
    satisfying_password_strength(password)
        .then_some(())
        .ok_or_else(|| RegisterError::PasswordTooWeak)?;

    let otp_res = io.request_otp(OtpRequest { count: 2 }).await?;

    let otps = otp_res
        .otps
        .try_into()
        .map_err(|_| anyhow!("Invalid otp response. Expected 2 one time passwords."))?;

    let hashed_hw_id = machine_uid()?;

    // prepare register data
    let register_data = register_data(otps, hashed_hw_id, email, password)?;

    // Tell the account server about our plan to register
    let response = send_func(register_data.for_server).await?;

    handle_register_res(response, register_data.for_client, password, io.io).await
}
