use serde::{Deserialize, Serialize};

use super::register::RegisterResponse;

/// The response after a successful password/account reset.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PasswordResetResponse {
    /// Since a password reset is basically also an
    /// account reset, it is similar to a register process
    /// and thus the types are shared.
    pub register_res: RegisterResponse,
}
