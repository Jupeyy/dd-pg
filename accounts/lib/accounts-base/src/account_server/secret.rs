use aes_gcm_siv::AeadCore;
use rand::Rng;
use serde::{Deserialize, Serialize};

/// A secret that is used to encrypt the main secret on
/// the client.  
/// __IMPORTANT__: This secret can be used to encrypt/decrypt
/// __EXACTLY__ one unique file (usually the main secret).  
/// __!!Never use an object of [AccountServerSecret] twice!!__
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountServerSecret {
    /// This is the secret the server generated
    /// which the client can use to encrypt
    /// files without storing the secret locally.
    /// __IMPORTANT__: never store/send this secret
    /// anywhere from the client. Decrypt the file
    /// and remove it from memory.  
    /// __!!Never use it twice for two different files!!__
    pub secret: [u8; 32],
    /// A randomly generated nonce  
    /// __!!Never use it twice for two different files!!__
    pub nonce: aes_gcm_siv::Nonce,
}

/// Generates a random [`AccountServerSecret`].
pub fn generate_account_server_secret() -> AccountServerSecret {
    AccountServerSecret {
        nonce: aes_gcm_siv::Aes256GcmSiv::generate_nonce(&mut rand::rngs::OsRng),
        secret: rand::rngs::OsRng.gen(),
    }
}
