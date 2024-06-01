use serde::{Deserialize, Serialize};

/// The password forget request type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PasswordForgotRequest {
    /// An email to send the recovery to
    pub email: email_address::EmailAddress,
}
