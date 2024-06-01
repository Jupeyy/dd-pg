use url::Url;

use crate::{
    email::EmailShared, mysql::MySqlConnectionShared, otps::Otps, register_tokens::RegisterTokens,
};

/// Shared data across the implementation
#[derive(Debug)]
pub struct Shared {
    pub mysql: MySqlConnectionShared,
    pub otps: Otps,
    pub register_tokens: RegisterTokens,
    pub email: EmailShared,
    /// the claimed URL of the account server,
    /// this is used in emails sent to the user
    pub http_url: Url,
    /// An admin password used to allow certain actions
    /// like verifying an account as game server group.
    pub admin_password: String,
}
