use chrono::{DateTime, Utc};
use ed25519_dalek::{ed25519::signature::Signer, Signature, SigningKey, VerifyingKey};
use serde::{Deserialize, Serialize};

/// Represents an auth request the client
/// sends to the account server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignRequest {
    /// The public key is signed using the client's
    /// private session key.
    pub pub_key: VerifyingKey,
    /// The timestamp when the sign request was triggered
    pub time_stamp: DateTime<Utc>,
    /// The signature for the above time stamp
    pub signature: Signature,
    /// Some kind of unique and non changing id
    /// obtained for this computer.  
    /// This slightly increases security,
    /// because an attacker might have a harder time to
    /// obtain such an id from the user's computer
    pub hw_id: [u8; 32],
}

/// Generate data for an sign request
pub fn prepare_sign_request(
    hw_id: [u8; 32],
    key: &SigningKey,
    pub_key: VerifyingKey,
) -> SignRequest {
    let time_stamp = chrono::Utc::now();
    let time_str = time_stamp.to_string();

    let signature = key.sign(time_str.as_bytes());

    SignRequest {
        pub_key,
        signature,
        time_stamp,
        hw_id,
    }
}
