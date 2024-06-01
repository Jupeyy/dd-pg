use ed25519_dalek::{VerifyingKey, PUBLIC_KEY_LENGTH};

/// A type that represents an user id
pub type UserId = [u8; PUBLIC_KEY_LENGTH];

/// Get the user id from a public key send by a client
pub fn user_id_from_pub_key(public_key: VerifyingKey) -> UserId {
    public_key.to_bytes()
}
