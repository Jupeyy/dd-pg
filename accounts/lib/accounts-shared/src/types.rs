use generic_array::{typenum::U12, GenericArray};
use serde::{Deserialize, Serialize};

/// A one time number, mostly used for encryption.
pub type Nonce = GenericArray<u8, U12>;

/// Represents a ed25519 private key
/// which was encrypted using [`aes_gcm_siv`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedPrivateKey {
    /// The encrypted private key
    pub key: Vec<u8>,
    /// A random nonce used to encrypt the key
    pub nonce: Nonce,
}

/// Represents a main secret that encrypts and decrypts
/// game server group key-pairs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedMainSecretWithServerSecret {
    /// The encrypted secret
    pub secret: Vec<u8>,
}

/// Represents a main secret to decrypt
/// the keys of the game servers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedMainSecret {
    /// The encrypted secret to decrypt private keys
    pub secret: Vec<u8>,
    /// A random nonce used to encrypt the key
    pub nonce: Nonce,
    /// The salt string used to hash the client's
    /// password, which (the hash) then was used
    /// to encrypt the private key.
    pub client_password_salt: String,
}

/// Bytes that were salted and hashed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaltedHashedBytes {
    /// The hash of the password.
    pub hash: [u8; 32],
    /// A salt that was used to hash the bytes.
    pub salt: String,
}

/// A password that was salted and hashed.
pub type SaltedHashedPassword = SaltedHashedBytes;
