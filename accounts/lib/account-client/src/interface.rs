use accounts_base::game_server::server_id::GameServerGroupId;
use async_trait::async_trait;

use crate::errors::{FsLikeError, HttpLikeError};

/// An io interface for the client to abstract away
/// the _actual_ communication used to communicate
/// with the account server.
#[async_trait]
pub trait Io: Sync + Send {
    /// Requests an one time password from the account server.
    /// Sends & receives it as arbitrary data.
    async fn request_otp(&self, data: Vec<u8>) -> anyhow::Result<Vec<u8>, HttpLikeError>;
    /// Requests an one time token from the account server, that
    /// can be sent to the game server to verify the account id of
    /// the client on the game server.
    /// Sends & receives as arbitrary data.
    async fn request_register_token(&self, data: Vec<u8>)
        -> anyhow::Result<Vec<u8>, HttpLikeError>;
    /// Requests a password reset for a given email. Where the request data and response data
    /// are arbitrary.
    async fn request_password_forgot(
        &self,
        data: Vec<u8>,
    ) -> anyhow::Result<Vec<u8>, HttpLikeError>;
    /// Request the key-pair of a game server group stored on the account server (if available).
    /// The request data and response data are arbitrary.
    async fn request_game_server_group_key_pair(
        &self,
        data: Vec<u8>,
    ) -> anyhow::Result<Vec<u8>, HttpLikeError>;
    /// Request to store the encrypted key-pair of a game server group stored on the account server.
    /// The request data and response data are arbitrary.
    async fn request_store_game_server_group_key_pair(
        &self,
        data: Vec<u8>,
    ) -> anyhow::Result<Vec<u8>, HttpLikeError>;
    /// Send to actually do a password reset with a given reset token. Where the request data and response data
    /// are arbitrary.
    async fn send_password_reset(&self, data: Vec<u8>) -> anyhow::Result<Vec<u8>, HttpLikeError>;
    /// Sends the register request as arbitrary data. Receives the response as arbitrary data.
    async fn send_register(&self, data: Vec<u8>) -> anyhow::Result<Vec<u8>, HttpLikeError>;
    /// Sends an auth request as arbitrary data. Receives the response as arbitrary data.
    async fn send_auth(&self, data: Vec<u8>) -> anyhow::Result<Vec<u8>, HttpLikeError>;
    /// Sends a login request as arbitrary data. Receives the response as arbitrary data.
    async fn send_login(&self, data: Vec<u8>) -> anyhow::Result<Vec<u8>, HttpLikeError>;
    /// Gets the account id for a given register token.
    /// Request & response data are arbitrary.
    async fn account_id_of_register_token(
        &self,
        data: Vec<u8>,
    ) -> anyhow::Result<Vec<u8>, HttpLikeError>;
    /// Write the encrypted main secret file to persistent secure
    /// (at least obviously named like `password_data`)
    /// storage on the client.
    /// Note: the file is not compressed, just serialized.
    async fn write_encrypted_main_secret_file(
        &self,
        file: Vec<u8>,
    ) -> anyhow::Result<(), FsLikeError>;
    /// Read the encrypted main secret file from the secure persistent storage
    /// on the client, previously written by [`Io::write_encrypted_main_secret_file`].
    /// Note: the file must not be compressed, just serialized.
    async fn read_encrypted_main_secret_file(&self) -> anyhow::Result<Vec<u8>, FsLikeError>;
    /// Write the game server group's key-pair file, which is sent to game servers,
    /// to persistent storage on the client.
    /// It's recommended to use the `game_server_group_id` in lowercase hex form
    /// to reidentify this file (if `Some` was used).
    /// A value of `None` means that a game server uses this API.
    /// Note: the file is not compressed, just serialized.
    async fn write_game_server_group_key_pair_file(
        &self,
        game_server_group_id: Option<GameServerGroupId>,
        file: Vec<u8>,
    ) -> anyhow::Result<(), FsLikeError>;
    /// Read the game server group's key-pair file, which is sent to game servers,
    /// from the persistent storage on the client, previously
    /// written by [`Io::write_game_server_group_key_pair_file`].
    /// It's recommended to use the `game_server_group_id` in lowercase hex form
    /// to reidentify this file (if `Some` was used).
    /// A value of `None` means that a game server uses this API.
    /// Note: the file must not be compressed, just serialized.
    async fn read_game_server_group_key_pair_file(
        &self,
        game_server_group_id: Option<GameServerGroupId>,
    ) -> anyhow::Result<Vec<u8>, FsLikeError>;
    /// Write the encrypted main secret file, which was encrypted using a secret
    /// from the account server, to persistent secure
    /// (at least obviously named like `password_data`)
    /// storage on the client.
    /// Note: This file should not be confused with the files written by
    /// [`Io::read_encrypted_main_secret_file`] or [`Io::write_session_key_pair_file`].
    /// It is a different file and should also be stored differently.
    /// Note: the file is not compressed, just serialized.
    async fn write_server_encrypted_main_secret_file(
        &self,
        file: Vec<u8>,
    ) -> anyhow::Result<(), FsLikeError>;
    /// Read the encrypted main secret file, which was encrypted with a server secret,
    /// from the secure persistent storage on the client,
    /// previously written by [`Io::write_server_encrypted_main_secret_file`].
    /// Note: This file should not be confused with the files written by
    /// [`Io::read_encrypted_main_secret_file`] or [`Io::write_session_key_pair_file`].
    /// Note: the file must not be compressed, just serialized.
    async fn read_server_encrypted_main_secret_file(&self) -> anyhow::Result<Vec<u8>, FsLikeError>;
    /// Write the private key used for a session to the account server
    /// to persistent secure (at least obviously named like `password_data`)
    /// storage on the client.
    /// Note: the file is not compressed, just serialized.
    async fn write_session_key_pair_file(&self, file: Vec<u8>) -> anyhow::Result<(), FsLikeError>;
    /// Read the private key of the session previously written to persistent storage
    /// by [`Io::write_session_key_pair_file`]
    /// Note: the file must not be compressed, just serialized.
    async fn read_session_key_pair_file(&self) -> anyhow::Result<Vec<u8>, FsLikeError>;
}
