use serde::{Deserialize, Serialize};

/// A request from the client to obtain one time passwords
/// from the account server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OtpRequest {
    /// The amount of one time passwords required,
    /// note that this is strictly limited on the account server.
    /// Currently the maximum a client can request is 3.
    pub count: u8,
}
