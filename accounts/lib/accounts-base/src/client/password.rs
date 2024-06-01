use anyhow::anyhow;
use argon2::{
    password_hash::{Salt, SaltString},
    Argon2, PasswordHasher,
};

use crate::types::{SaltedHashedBytes, SaltedHashedPassword};

/// Generates a hash for the given bytes with the given salt
/// using argon2.
///
/// # Errors
/// Only throws errors if a crypto function failed unexpected.
pub fn argon2_hash_from_salt(bytes: &[u8], salt: Salt<'_>) -> anyhow::Result<[u8; 32]> {
    // Hashed bytes salted as described above
    let argon2 = Argon2::default();
    Ok(argon2
        .hash_password(bytes, salt)
        .map_err(|err| anyhow!(err))?
        .hash
        .ok_or_else(|| anyhow!("Hash was not valid"))?
        .as_bytes()
        .try_into()?)
}

/// Generates a hash for the given bytes with the given unsecure salt
/// using argon2.
/// Should only be used to hash things that are already secure in itself.
///
/// # Errors
/// Only throws errors if a crypto function failed unexpected.
pub fn argon2_hash_from_unsecure_salt(
    bytes: &[u8],
    unsecure_salt: String,
) -> anyhow::Result<[u8; 32]> {
    argon2_hash_from_salt(
        bytes,
        SaltString::encode_b64(unsecure_salt.as_bytes())
            .map_err(|err| anyhow!(err))?
            .as_salt(),
    )
}

/// Generates a password hash with the given salt
/// using argon2 and returns a argon2 password hash.
///
/// # Errors
/// Only throws errors if a crypto function failed unexpected.
pub fn argon2_hash_password_from_salt(password: &str, salt: Salt<'_>) -> anyhow::Result<[u8; 32]> {
    let password = password.as_bytes();

    argon2_hash_from_salt(password, salt)
}

/// Generates a secure random salt
pub fn generate_salt() -> SaltString {
    let mut rng = rand::rngs::OsRng;

    // Generates the salt that is used to hash the local password
    // which is sent to the server.
    // This salt is included in that process and only exist to make
    // identifying similar passwords harder.
    SaltString::generate(&mut rng)
}

/// Hashes the bytes using [`argon2`] with a random salt.
/// The result contains the salted hash and the salt.
///
/// # Errors
/// Only throws errors if a crypto function failed unexpected.
pub fn hash_bytes(bytes: &[u8]) -> anyhow::Result<SaltedHashedBytes> {
    let salt = generate_salt();
    let hash = argon2_hash_from_salt(bytes, salt.as_salt())?;

    Ok(SaltedHashedBytes {
        hash,
        salt: salt.as_str().to_string(),
    })
}

/// Hashes the password using [`argon2`] with a random salt.
/// The result contains the salted password hash and the salt.
///
/// # Errors
/// Only throws errors if a crypto function failed unexpected.
pub fn hash_password(password: &str) -> anyhow::Result<SaltedHashedPassword> {
    let salt = generate_salt();
    let hash = argon2_hash_password_from_salt(password, salt.as_salt())?;

    Ok(SaltedHashedPassword {
        hash,
        salt: salt.as_str().to_string(),
    })
}

pub use zxcvbn::Score;

/// Small helper the client can implement to get the strength of the
/// user password.
/// See [`zxcvbn::Entropy`] for more information about the result,
/// usually calling `score` on the result is a good indicator of
/// the overall strength.
pub fn password_strength(password: &str) -> zxcvbn::Entropy {
    zxcvbn::zxcvbn(password, &[])
}
