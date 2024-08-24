use async_trait::async_trait;

use crate::errors::{FsLikeError, HttpLikeError};

/// An io interface for the client to abstract away
/// the _actual_ communication used to communicate
/// with the account server.
#[async_trait]
pub trait Io: Sync + Send {
    /// Requests an one time password from the account server for the given email.
    /// Sends & receives it as arbitrary data.
    async fn request_login_email_token(
        &self,
        data: Vec<u8>,
    ) -> anyhow::Result<Vec<u8>, HttpLikeError>;
    /// Requests a login for the given account.
    /// Sends & receives it as arbitrary data.
    async fn request_login(&self, data: Vec<u8>) -> anyhow::Result<Vec<u8>, HttpLikeError>;
    /// Requests a logout for the given session.
    /// Sends & receives it as arbitrary data.
    async fn request_logout(&self, data: Vec<u8>) -> anyhow::Result<(), HttpLikeError>;
    /// Requests the account server to sign a certificate.
    /// Sends & receives it as arbitrary data.
    async fn request_sign(&self, data: Vec<u8>) -> anyhow::Result<Vec<u8>, HttpLikeError>;
    /// Requests an one time password from the account server for the given email.
    /// Sends & receives it as arbitrary data.
    async fn request_account_token_email(&self, data: Vec<u8>)
        -> anyhow::Result<(), HttpLikeError>;
    /// Requests to delete all session for the given account.
    /// Sends & receives it as arbitrary data.
    async fn request_delete_sessions(&self, data: Vec<u8>) -> anyhow::Result<(), HttpLikeError>;
    /// Requests to delete an account.
    /// Sends & receives it as arbitrary data.
    async fn request_delete_account(&self, data: Vec<u8>) -> anyhow::Result<(), HttpLikeError>;
    /// Downloads the latest certificates of the account server.
    /// Sends & receives it as arbitrary data.
    async fn download_account_server_certificates(&self) -> anyhow::Result<Vec<u8>, HttpLikeError>;
    /// Write the serialized session key pair to a secure storage
    /// (at least obviously named like `password_data`)
    /// on the client.
    /// Note: the file is not compressed, just serialized.
    async fn write_serialized_session_key_pair(
        &self,
        file: Vec<u8>,
    ) -> anyhow::Result<(), FsLikeError>;
    /// Read the serialized session key pair from storage
    /// on the client, previously written by [`Io::write_serialized_session_key_pair`].
    /// Note: the file must not be compressed, just serialized.
    async fn read_serialized_session_key_pair(&self) -> anyhow::Result<Vec<u8>, FsLikeError>;
    /// Remove the session keys from file disk
    async fn remove_serialized_session_key_pair(&self) -> anyhow::Result<(), FsLikeError>;
}
