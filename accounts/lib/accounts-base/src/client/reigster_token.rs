use serde::{Deserialize, Serialize};

use super::auth::AuthRequest;

/// The request a register token from the account
/// server using an auth request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterTokenRequest {
    /// The email to use to query the client salt
    pub auth_req: AuthRequest,
}
