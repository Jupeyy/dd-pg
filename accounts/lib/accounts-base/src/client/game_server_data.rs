use crate::types::EncryptedPrivateKey;
use aes_gcm_siv::{aead::Aead, AeadCore, KeyInit, Nonce};
use anyhow::anyhow;
use ed25519_dalek::{SecretKey, SigningKey, VerifyingKey};
use serde::{Deserialize, Serialize};

use crate::game_server::server_id::GameServerGroupId;

use super::auth::AuthRequest;

/// A game server key pair, where the private key
/// is encrypted.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameServerKeyPair {
    /// A ed25519 private key that is used to to generate
    /// a signature to identify the user on a game server
    /// (not account server).
    /// It is in an encrypted form and only accessable by using the password
    pub priv_key: EncryptedPrivateKey,
    /// The public key the client should send to game servers.
    pub pub_key: VerifyingKey,
}

/// The unencrypted key-pair for the game server group.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientGameServerKeyPair {
    /// The private key to sign messages the are sent by the client
    /// to a game server.  
    /// __NEVER__ share or save this private key anywhere.
    pub private_key: SigningKey,
    /// This key should be send to the game server to verify
    /// the signature of messages sent by the client to the
    /// game server.
    pub public_key: VerifyingKey,
}

/// This is the game server data that should be sent to the account server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameServerDataForAccountServer {
    /// The encrypted key-pair for this game server group
    pub key_pair: GameServerKeyPair,
}

/// The secret data the client must store and should not share with anyone
#[derive(Debug)]
pub struct GameServerDataForClient {
    /// The encrypted key-pair for this game server group
    pub key_pair: GameServerKeyPair,
}

/// The result type for [`generate_account_data`].
/// Contains everything that is required to register a new account
/// or to change a password on client & server.
#[derive(Debug)]
pub struct GameServerData {
    /// Data that should be send to the account server,
    /// see [`GameServerDataForAccountServer`]
    pub for_server: GameServerDataForAccountServer,
    /// Data that should be kept secret on the client,
    /// see [`GameServerDataForClient`]
    pub for_client: GameServerDataForClient,
}

/// The request for a game server group key-pair
/// from the account server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestGameServerKeyPair {
    /// Auth data to verify the user can get the key pair.
    pub auth_data: AuthRequest,
    /// The id of the game server group, for whom the key-pair was created.
    /// If `None`, the account server assumes this is a game server.
    pub game_server_group_id: Option<GameServerGroupId>,
}

/// The request to store a key-pair for a game server group
/// on the account server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestStoreGameServerKeyPair {
    /// Auth data to verify the user can store the key pair.
    pub auth_data: AuthRequest,
    /// The encrypted key-pair.
    pub key_pair: GameServerDataForAccountServer,
    /// The id of the game server group, for whom the key-pair was created.
    /// If `None`, the account server assumes this is a game server.
    pub game_server_group_id: Option<GameServerGroupId>,
}

/// Generates a key pair to use for a game server.
///
/// # Errors
/// Only returns errors if one of the crypto functions
/// failed to execute.
pub fn generate_client_game_server_key_pair() -> anyhow::Result<ClientGameServerKeyPair> {
    let mut rng = rand::rngs::OsRng;

    // The private key of a server id
    let priv_key = SigningKey::generate(&mut rng);
    let pub_key = priv_key.verifying_key();

    Ok(ClientGameServerKeyPair {
        private_key: priv_key,
        public_key: pub_key,
    })
}

/// This generates new game server group data using the master secret to encrypt it.
/// In other words, this function returns an encrypted
/// private key aswell as the public key
/// to store in the client and the account server.
///
/// # Errors
/// Only returns errors if one of the crypto functions
/// failed to execute.
pub fn generate_game_server_group_data(main_secret: &[u8]) -> anyhow::Result<GameServerData> {
    let key_pair = generate_client_game_server_key_pair()?;

    // A random nonce is used
    let mut rng = rand::rngs::OsRng;
    let nonce = aes_gcm_siv::Aes256GcmSiv::generate_nonce(&mut rng);
    let cipher = aes_gcm_siv::Aes256GcmSiv::new_from_slice(main_secret)?;
    let encrypted_priv_key = cipher
        .encrypt(&nonce, key_pair.private_key.as_bytes().as_slice())
        .map_err(|err| anyhow!(err))?;

    // This is not allowed to be used anymore after encrypt
    drop(key_pair.private_key);

    let priv_key = EncryptedPrivateKey {
        key: encrypted_priv_key,
        nonce,
    };

    let key_pair = GameServerKeyPair {
        priv_key,
        pub_key: key_pair.public_key,
    };

    Ok(GameServerData {
        for_server: GameServerDataForAccountServer {
            key_pair: key_pair.clone(),
        },
        for_client: GameServerDataForClient { key_pair },
    })
}

/// Decrypts an encrypted game server group key-pair using the master secret.
pub fn decrypt_priv_key_from_main_secret(
    key: EncryptedPrivateKey,
    main_secret: &[u8],
) -> anyhow::Result<SigningKey> {
    let nonce = Nonce::from_slice(&key.nonce);
    let cipher = aes_gcm_siv::Aes256GcmSiv::new_from_slice(main_secret)?;
    let decrypted_priv_key = cipher
        .decrypt(nonce, key.key.as_slice())
        .map_err(|err| anyhow!("failed to decrypt key: {err}"))?;

    let secret_key = SecretKey::try_from(decrypted_priv_key)
        .map_err(|_| anyhow!("decrypted private key was not a ed25519 private key"))?;
    Ok(SigningKey::from_bytes(&secret_key))
}
