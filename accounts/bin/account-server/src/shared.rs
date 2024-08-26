use std::sync::Arc;

use parking_lot::RwLock;

use crate::{certs::PrivateKeys, db::DbConnectionShared, email::EmailShared};

/// Shared data across the implementation
pub struct Shared {
    pub db: DbConnectionShared,
    pub email: EmailShared,
    /// A signing key to sign the certificates for the account users.
    pub signing_keys: Arc<RwLock<Arc<PrivateKeys>>>,
    /// All certificates that are valid for any certificate generated
    /// by any legit account server.
    pub cert_chain: Arc<RwLock<Arc<Vec<x509_cert::Certificate>>>>,
}
