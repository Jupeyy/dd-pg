use aes_gcm_siv::{aead::Aead, AeadCore, KeyInit, Nonce};
use anyhow::anyhow;
use argon2::password_hash::SaltString;
use rand::Rng;
use serde::{Deserialize, Serialize};

use crate::types::EncryptedMainSecret;

use super::password::{
    argon2_hash_from_unsecure_salt, argon2_hash_password_from_salt, hash_password,
};

/// This is the account data that should be sent to the server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountDataForServer {
    /// [`argon2`] pre-hashed password
    pub hashed_password: [u8; 32],
    /// Encrypted secret to store on the server.
    /// This is the same secret that is also in
    /// [`AccountDataForClient`].
    /// The reason the server gets this is, that
    /// the server acts as managed storage.
    pub secret: EncryptedMainSecret,
}

/// The secret data the client must store and should not share with anyone
#[derive(Debug)]
pub struct AccountDataForClient {
    /// This secret encrypts and decrypts the private keys
    /// that are used to authenticate against the game servers.
    /// The secret itself is encrypted and must be decrypted using
    /// the account's real password.
    pub secret: EncryptedMainSecret,
}

/// The result type for [`generate_account_data`].
/// Contains everything that is required to register a new account
/// or to change a password on client & server.
#[derive(Debug)]
pub struct AccountData {
    /// Data that should be send to the server, see [`AccountDataForServer`]
    pub for_server: AccountDataForServer,
    /// Data that should be kept secret on the client, see [`AccountDataForClient`]
    pub for_client: AccountDataForClient,
}

/// This generates new account data from a new password.
/// In other words, this function returns an encrypted
/// main secret aswell as the password in a hashed form
/// for the account server and the encrypted, using the password,
/// main secret to store in the client.
///
/// # Errors
/// Only returns errors if one of the crypto functions
/// failed to execute.
pub fn generate_account_data(
    email: &email_address::EmailAddress,
    password: &str,
) -> anyhow::Result<AccountData> {
    let hashed_email =
        argon2_hash_from_unsecure_salt(email.as_str().as_bytes(), "ddnet-accounts".into())?;
    let hashed_password = argon2_hash_password_from_salt(
        password,
        SaltString::encode_b64(&hashed_email)
            .map_err(|err| anyhow!(err))?
            .as_salt(),
    )?;

    let mut rng = rand::rngs::OsRng;

    // The main secret for the client's keys
    let mut main_secret: [u8; 32] = rng.gen();

    // The client side password is salted to stretch it for weaker password,
    // but this salt is saved together with the main secret.
    // It is used to encrypt the main secret, so the server never knows
    // about the actual main secret. It's not salted, else the client would
    // need to remember it.
    let hashed_pw_for_encrypt = hash_password(password)?;

    // A random nonce is used
    let nonce = aes_gcm_siv::Aes256GcmSiv::generate_nonce(&mut rng);
    let cipher = aes_gcm_siv::Aes256GcmSiv::new_from_slice(hashed_pw_for_encrypt.hash.as_slice())?;
    let encrypted_main_secret = cipher
        .encrypt(&nonce, main_secret.as_slice())
        .map_err(|err| anyhow!(err))?;

    // This is not allowed to be used anymore after encrypt, remove it from memory
    main_secret.fill(0);

    let main_secret = EncryptedMainSecret {
        secret: encrypted_main_secret,
        nonce,
        client_password_salt: hashed_pw_for_encrypt.salt,
    };

    Ok(AccountData {
        for_server: AccountDataForServer {
            hashed_password,
            secret: main_secret.clone(),
        },
        for_client: AccountDataForClient {
            secret: main_secret,
        },
    })
}

/// Decrypts an encrypted main secret using a password.
pub fn decrypt_main_secret_from_password(
    main_secret: EncryptedMainSecret,
    password: &str,
) -> anyhow::Result<Vec<u8>> {
    let salt = SaltString::from_b64(main_secret.client_password_salt.as_str())
        .map_err(|err| anyhow!(err))?;
    let hashed_pw_for_decrypt = argon2_hash_password_from_salt(password, salt.as_salt())?;

    let nonce = Nonce::from_slice(&main_secret.nonce);
    let cipher = aes_gcm_siv::Aes256GcmSiv::new_from_slice(&hashed_pw_for_decrypt)?;
    let decrypted_main_secret = cipher
        .decrypt(nonce, main_secret.secret.as_slice())
        .map_err(|err| anyhow!(err))?;

    Ok(decrypted_main_secret)
}
