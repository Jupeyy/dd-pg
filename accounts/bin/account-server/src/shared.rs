use p256::ecdsa::SigningKey;

use crate::{email::EmailShared, mysql::MySqlConnectionShared};

/// Shared data across the implementation
#[derive(Debug)]
pub struct Shared {
    pub mysql: MySqlConnectionShared,
    pub email: EmailShared,
    /// A signing key to sign the certificates for the account users.
    pub signing_key: SigningKey,
}
