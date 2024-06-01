use serde::{Deserialize, Serialize};

use super::{account_id::AccountId, secret::AccountServerSecret};

/// The secret that the client can use to decrypt
/// the main secret.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthResponseSecret {
    /// A secret from the account server
    /// that allows the client to decrypt
    /// the main secret used to decrypt
    /// game server group key-pairs.
    pub secret: AccountServerSecret,
}

/// Contains data for if the auth was successful and the client's account
/// is verified.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthResponseVerified {
    /// The secret to decrypt the account's main secret
    /// See [`AuthResponseSecret`].
    pub secret: AuthResponseSecret,
    /// The account id on the account server.
    pub account_id: AccountId,
}

/// The success response of an auth request from the client.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuthResponseSuccess {
    /// The account exists and is verified.
    Verified(AuthResponseVerified),
    /// The account exists, but is not verified.
    NotVerified(AuthResponseSecret),
}

impl AuthResponseSuccess {
    /// Get the auth secret
    pub const fn secret(&self) -> &AuthResponseSecret {
        match self {
            Self::Verified(secret) => &secret.secret,
            Self::NotVerified(secret) => secret,
        }
    }
}

/// The response of an auth request from the client.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuthResponse {
    /// The account server successfully
    /// authed the client.
    Success(AuthResponseSuccess),
    /// The auth request was invalid, the client
    /// has to issue a login.
    Invalid,
}
