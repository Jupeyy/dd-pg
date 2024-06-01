use serde::{Deserialize, Serialize};

use super::auth::AuthResponse;

/// After a client send a register request, the server
/// processes the request and answers with this response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RegisterResponse {
    /// Registering worked
    Success {
        /// Notify the client that even tho the account
        /// is registered, the server wants additional verification
        /// e.g. by email.
        requires_verification: bool,
        /// This is the auth response for the session that was
        /// automatically created in the process of registering.
        auth_response: AuthResponse,
    },
    /// Account with that email already exist
    AccountWithEmailAlreadyExists,
}
