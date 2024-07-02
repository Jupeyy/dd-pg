use anyhow::anyhow;
use base64::Engine;
use ed25519_dalek::{Signature, Signer};
use serde::{Deserialize, Serialize};

use super::account_data::{generate_account_data, AccountDataForClient, AccountDataForServer};

/// The supported types of verifications for an account's
/// session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LoginToken {
    /// A login token that was sent to an email
    Email([u8; 32]),
    /// A login token that was generated using steam's session tokens
    Steam([u8; 32]),
}

impl LoginToken {
    /// token as byte slice
    pub const fn as_slice(&self) -> &[u8; 32] {
        match self {
            Self::Email(token) => token,
            Self::Steam(token) => token,
        }
    }
}

/// Represents the data required for a login attempt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginRequest {
    /// The account data related to the login request.
    pub account_data: AccountDataForServer,
    /// A login token that was sent by
    /// email or generated for a steam based login etc.
    pub login_token: LoginToken,
    /// The signature for the login token,
    /// used to make sure the public key corresponds
    /// to a valid private key.
    pub login_token_signature: Signature,
}

/// Prepares a login attempt on the account server.
pub fn login_email(
    login_token_b64: String,
) -> anyhow::Result<(LoginRequest, AccountDataForClient)> {
    let login_token = base64::prelude::BASE64_URL_SAFE.decode(login_token_b64)?;

    let account_data = generate_account_data()?;

    let signature = account_data.for_client.private_key.sign(&login_token);

    Ok((
        LoginRequest {
            login_token_signature: signature,
            account_data: account_data.for_server,
            login_token: LoginToken::Email(
                login_token
                    .try_into()
                    .map_err(|_| anyhow!("Invalid login token."))?,
            ),
        },
        account_data.for_client,
    ))
}
