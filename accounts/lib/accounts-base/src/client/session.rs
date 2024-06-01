use aes_gcm_siv::{aead::Aead, KeyInit, Nonce};
use anyhow::anyhow;
use argon2::password_hash::SaltString;
use ed25519_dalek::{ed25519::signature::SignerMut, Signature, SigningKey, VerifyingKey};
use serde::{Deserialize, Serialize};

use super::{
    account_data::decrypt_main_secret_from_password,
    auth::{prepare_auth_request, AuthRequest},
    password::{argon2_hash_from_unsecure_salt, argon2_hash_password_from_salt},
};
use crate::account_server::{otp::Otp, secret::AccountServerSecret};
use crate::types::{EncryptedMainSecret, EncryptedMainSecretWithServerSecret};

/// This is the session data that should be sent to the server.
/// It's a superset of an [`AuthRequest`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionDataForServer {
    /// The email address in a email-compatible format
    pub email: email_address::EmailAddress,
    /// pre-hashed password so the server can check
    /// if our session request is valid.
    pub hashed_password: [u8; 32],
    /// A public key derived from a ed25519 private key
    /// used on the server to identify the client's session
    pub pub_key: VerifyingKey,
    /// A one time password that has to be signed using the client's
    /// private session key
    pub otp: Otp,
    /// The signature for the above password
    pub signature: Signature,
    /// Some kind of unique and non changing id
    /// obtained for this computer.  
    /// This slightly increases security,
    /// because an attacker might have a harder time to
    /// obtain such an id from the user's computer
    pub hw_id: [u8; 32],
    /// A session directly issues an auth request.
    /// This is all required data.
    pub auth_request: AuthRequest,
}

/// The secret data the client must store and should not share with anyone
#[derive(Debug)]
pub struct SessionDataForClient {
    /// A ed25519 private key that is used to to generate
    /// a signature to identify the user's session on the account server.  
    /// __!WARNING!__: Never share this key with anyone. It's only intended
    /// to be stored on __one__ of the client's computer. And not even shared between two
    /// computers of the same person.
    pub priv_key: SigningKey,
    /// A ed25519 public key, which is sent to the account server and signed
    /// to auth the user's session.
    pub pub_key: VerifyingKey,
}

/// The result type for [`generate_session_data`].
/// Contains everything that is needed to create a session on
/// client & server.
#[derive(Debug)]
pub struct SessionData {
    /// Data that should be send to the server, see [`DataForServer`]
    pub for_server: SessionDataForServer,
    /// Data that should be kept secret on the client, see [`DataForClient`]
    pub for_client: SessionDataForClient,
}

/// This generates all data required to
/// establish a session on the server, it returns
/// all data required to be sent to the server
/// and stored in the client so the server can identify
/// the client.
///
/// # Errors
/// Only returns errors if one of the crypto functions
/// failed to execute.
pub fn generate_session_data(
    login_otp: Otp,
    auth_otp: Otp,
    hw_id: [u8; 32],
    email: email_address::EmailAddress,
    password: &str,
) -> anyhow::Result<SessionData> {
    let hashed_email =
        argon2_hash_from_unsecure_salt(email.as_str().as_bytes(), "ddnet-accounts".into())?;
    let password_hash = argon2_hash_password_from_salt(
        password,
        SaltString::encode_b64(&hashed_email)
            .map_err(|err| anyhow!(err))?
            .as_salt(),
    )?;

    // This key-pair is similar to a session token
    // The client "registers" a pub-key on the server which the server
    // uses to identify the client's session private key.
    let mut rng = rand::rngs::OsRng;
    let mut priv_key = SigningKey::generate(&mut rng);
    let pub_key = priv_key.verifying_key();
    let signature = priv_key.sign(&login_otp);

    // the auth request data
    let auth_request = prepare_auth_request(auth_otp, hw_id, &mut priv_key, pub_key);

    Ok(SessionData {
        for_server: SessionDataForServer {
            email,
            hashed_password: password_hash,
            pub_key,
            auth_request,
            otp: login_otp,
            signature,
            hw_id,
        },
        for_client: SessionDataForClient { priv_key, pub_key },
    })
}

/// Transforms/reencrypts the accounts private secret to
/// with the server secret. This then allows
/// to auth on the account server to decrypt the
/// main secret instead of using the password.
pub fn reencrypt_main_secret_with_server_secret(
    main_secret: EncryptedMainSecret,
    password: &str,
    server_secret: AccountServerSecret,
) -> anyhow::Result<EncryptedMainSecretWithServerSecret> {
    let mut main_secret = decrypt_main_secret_from_password(main_secret, password)?;

    // A random nonce is used
    let nonce = Nonce::from_slice(&server_secret.nonce);
    let cipher = aes_gcm_siv::Aes256GcmSiv::new_from_slice(server_secret.secret.as_slice())?;
    let encrypted_main_secret = cipher
        .encrypt(nonce, main_secret.as_slice())
        .map_err(|err| anyhow!(err))?;

    // This is not allowed to be used anymore after encrypt
    main_secret.fill(0);
    drop(main_secret);

    Ok(EncryptedMainSecretWithServerSecret {
        secret: encrypted_main_secret,
    })
}
