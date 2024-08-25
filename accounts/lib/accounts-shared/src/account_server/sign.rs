use serde::{Deserialize, Serialize};

/// The response of an auth request from the client.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SignResponse {
    /// Signing a certificate was successful.
    Success {
        /// certificate, serialized in der format.
        cert_der: Vec<u8>,
    },
}
