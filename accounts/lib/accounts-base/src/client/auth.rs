use crate::types::EncryptedMainSecretWithServerSecret;
use aes_gcm_siv::{aead::Aead, KeyInit, Nonce};
use anyhow::anyhow;
use ed25519_dalek::{ed25519::signature::SignerMut, Signature, SigningKey, VerifyingKey};
use serde::{Deserialize, Serialize};

use crate::account_server::{auth::AuthResponseSecret, otp::Otp};

/// Represents an auth request the client
/// sends to the account server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthRequest {
    /// The public key is signed using the client's
    /// private session key.
    pub pub_key: VerifyingKey,
    /// An one time password used in the auth process
    pub otp: Otp,
    /// The signature for the above password
    pub signature: Signature,
    /// Some kind of unique and non changing id
    /// obtained for this computer.  
    /// This slightly increases security,
    /// because an attacker might have a harder time to
    /// obtain such an id from the user's computer
    pub hw_id: [u8; 32],
}

/// Generate data for an auth request
pub fn prepare_auth_request(
    otp: Otp,
    hw_id: [u8; 32],
    key: &mut SigningKey,
    pub_key: VerifyingKey,
) -> AuthRequest {
    let signature = key.sign(&otp);

    AuthRequest {
        pub_key,
        signature,
        otp,
        hw_id,
    }
}

/// Generate data for an auth request
pub fn decrypt_main_secret_from_auth_request(
    key_file: EncryptedMainSecretWithServerSecret,
    auth_res: AuthResponseSecret,
) -> anyhow::Result<Vec<u8>> {
    let nonce = Nonce::from_slice(&auth_res.secret.nonce);
    let cipher = aes_gcm_siv::Aes256GcmSiv::new_from_slice(auth_res.secret.secret.as_slice())?;
    let secret_bytes = cipher
        .decrypt(nonce, key_file.secret.as_slice())
        .map_err(|err| anyhow!(err))?;
    Ok(secret_bytes)
}
