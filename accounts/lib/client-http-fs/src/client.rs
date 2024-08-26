use std::sync::Arc;

use account_client::{
    errors::{FsLikeError, HttpLikeError},
    interface::Io,
};

use crate::{fs::Fs, http::Http};

#[derive(Debug)]
pub struct ClientHttpTokioFs {
    pub http: Arc<dyn Http>,
    pub fs: Fs,
}

#[async_trait::async_trait]
impl Io for ClientHttpTokioFs {
    async fn request_login_email_token(
        &self,
        data: Vec<u8>,
    ) -> anyhow::Result<Vec<u8>, HttpLikeError> {
        Ok(self
            .http
            .post_json(
                self.http
                    .base_url()
                    .join("/login/token-email")
                    .map_err(|err| HttpLikeError::Other(err.into()))?,
                data,
            )
            .await?)
    }
    async fn request_login(&self, data: Vec<u8>) -> anyhow::Result<Vec<u8>, HttpLikeError> {
        Ok(self
            .http
            .post_json(
                self.http
                    .base_url()
                    .join("/login/email")
                    .map_err(|err| HttpLikeError::Other(err.into()))?,
                data,
            )
            .await?)
    }
    async fn request_logout(&self, data: Vec<u8>) -> anyhow::Result<(), HttpLikeError> {
        Ok(self
            .http
            .post_json(
                self.http
                    .base_url()
                    .join("/logout")
                    .map_err(|err| HttpLikeError::Other(err.into()))?,
                data,
            )
            .await
            .map(|_| ())?)
    }
    async fn request_sign(&self, data: Vec<u8>) -> anyhow::Result<Vec<u8>, HttpLikeError> {
        Ok(self
            .http
            .post_json(
                self.http
                    .base_url()
                    .join("/sign")
                    .map_err(|err| HttpLikeError::Other(err.into()))?,
                data,
            )
            .await?)
    }
    async fn request_account_token_email(
        &self,
        data: Vec<u8>,
    ) -> anyhow::Result<(), HttpLikeError> {
        Ok(self
            .http
            .post_json(
                self.http
                    .base_url()
                    .join("/account-token")
                    .map_err(|err| HttpLikeError::Other(err.into()))?,
                data,
            )
            .await
            .map(|_| ())?)
    }
    async fn request_delete_sessions(&self, data: Vec<u8>) -> anyhow::Result<(), HttpLikeError> {
        Ok(self
            .http
            .post_json(
                self.http
                    .base_url()
                    .join("/delete-sessions")
                    .map_err(|err| HttpLikeError::Other(err.into()))?,
                data,
            )
            .await
            .map(|_| ())?)
    }
    async fn request_delete_account(&self, data: Vec<u8>) -> anyhow::Result<(), HttpLikeError> {
        Ok(self
            .http
            .post_json(
                self.http
                    .base_url()
                    .join("/delete-account")
                    .map_err(|err| HttpLikeError::Other(err.into()))?,
                data,
            )
            .await
            .map(|_| ())?)
    }
    async fn download_account_server_certificates(&self) -> anyhow::Result<Vec<u8>, HttpLikeError> {
        self.http
            .get(
                self.http
                    .base_url()
                    .join("/certs")
                    .map_err(|err| HttpLikeError::Other(err.into()))?,
            )
            .await
    }
    async fn write_serialized_session_key_pair(
        &self,
        file: Vec<u8>,
    ) -> anyhow::Result<(), FsLikeError> {
        self.fs.write("account.key".as_ref(), file).await
    }
    async fn read_serialized_session_key_pair(&self) -> anyhow::Result<Vec<u8>, FsLikeError> {
        self.fs.read("account.key".as_ref()).await
    }
    async fn remove_serialized_session_key_pair(&self) -> anyhow::Result<(), FsLikeError> {
        self.fs.remove("account.key".as_ref()).await
    }
}
