use serde::{Deserialize, Serialize};

/// The response of a login request by the client.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LoginResponse {
    /// Worked
    Success,
    /// Token invalid, probably timed out
    TokenInvalid,
}
