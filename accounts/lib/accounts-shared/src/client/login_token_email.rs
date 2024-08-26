use serde::{Deserialize, Serialize};

/// A request for a token that is used for the
/// email login.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginTokenEmailRequest {
    /// The email of the account to log into.
    pub email: email_address::EmailAddress,
}
