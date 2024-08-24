use serde::{Deserialize, Serialize};

/// A token previously sent to email or generated
/// for a steam account, that can be used to perform various
/// actions on an account, e.g. deleting it or removing/revoking
/// all active sessions.
pub type AccountToken = [u8; 32];

/// A request for an account token by email.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountTokenEmailRequest {
    /// The email of the account.
    pub email: email_address::EmailAddress,
}
