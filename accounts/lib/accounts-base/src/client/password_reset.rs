use serde::{Deserialize, Serialize};

use super::register::RegisterDataForServer;

/// The type to send a password reset request
/// to an account server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PasswordResetRequest {
    /// The reset code to send to the account server.
    /// Encoded into a base64 string.
    pub reset_code_base64: String,
    /// The new account data & session information.
    /// This is similar to registering, except that it
    /// won't create a new account id.
    pub register_data: RegisterDataForServer,
}
