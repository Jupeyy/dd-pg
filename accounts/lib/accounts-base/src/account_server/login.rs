use serde::{Deserialize, Serialize};

use crate::types::EncryptedMainSecret;

use super::auth::AuthResponseSuccess;

/// The success response of a login request by the client.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginResponseSuccess {
    /// The auth response if the login was sucessful,
    /// since a login also results in an auth automatically.
    pub auth: AuthResponseSuccess,
    /// This is the main secret stored on the account server.
    /// It is encrypted by the client using the password,
    /// so only the client can decrypt it.
    pub main_secret: EncryptedMainSecret,
}

/// The response of a login request by the client.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LoginResponse {
    /// The login into the account server was sucessful.
    Success(LoginResponseSuccess),
    /// An invalid email or password was sent,
    /// it's intentionally not clear which of the two.
    InvalidPasswordOrEmail,
}
