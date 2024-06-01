use serde::{Deserialize, Serialize};

use super::otp::{generate_otp, Otp};

/// A register token that the client requests from the account
/// server to verify on the game server that the account id
/// of the client is actually its.
pub type RegisterToken = Otp;

/// Generates a new random register token
pub fn generate_register_token() -> Otp {
    generate_otp()
}

/// The response for the client when the register
/// token request was successful.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterTokenResponse {
    /// The register token that the client
    /// can send to the game server.
    /// It's important that this token should be
    /// signed by the client's private key.
    pub token: RegisterToken,
}
