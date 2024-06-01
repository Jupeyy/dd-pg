use std::sync::Arc;

use account_client::{
    errors::{FsLikeError, HttpLikeError},
    interface::Io,
};
use accounts_base::game_server::server_id::GameServerGroupId;

use crate::{fs::Fs, http::Http};

#[derive(Debug)]
pub struct ClientHttpTokioFs {
    pub http: Arc<dyn Http>,
    pub fs: Fs,
}

#[async_trait::async_trait]
impl Io for ClientHttpTokioFs {
    async fn request_otp(&self, data: Vec<u8>) -> anyhow::Result<Vec<u8>, HttpLikeError> {
        Ok(self
            .http
            .post_json(
                self.http
                    .base_url()
                    .join("/otp")
                    .map_err(|err| HttpLikeError::Other(err.into()))?,
                data,
            )
            .await?)
    }
    async fn request_register_token(
        &self,
        data: Vec<u8>,
    ) -> anyhow::Result<Vec<u8>, HttpLikeError> {
        Ok(self
            .http
            .post_json(
                self.http
                    .base_url()
                    .join("/register-token")
                    .map_err(|err| HttpLikeError::Other(err.into()))?,
                data,
            )
            .await?)
    }
    async fn request_password_forgot(
        &self,
        data: Vec<u8>,
    ) -> anyhow::Result<Vec<u8>, HttpLikeError> {
        Ok(self
            .http
            .post_json(
                self.http
                    .base_url()
                    .join("/password-forgot")
                    .map_err(|err| HttpLikeError::Other(err.into()))?,
                data,
            )
            .await?)
    }
    async fn request_game_server_group_key_pair(
        &self,
        data: Vec<u8>,
    ) -> anyhow::Result<Vec<u8>, HttpLikeError> {
        Ok(self
            .http
            .post_json(
                self.http
                    .base_url()
                    .join("/server-group-key-pair")
                    .map_err(|err| HttpLikeError::Other(err.into()))?,
                data,
            )
            .await?)
    }
    async fn request_store_game_server_group_key_pair(
        &self,
        data: Vec<u8>,
    ) -> anyhow::Result<Vec<u8>, HttpLikeError> {
        Ok(self
            .http
            .post_json(
                self.http
                    .base_url()
                    .join("/store-server-group-key-pair")
                    .map_err(|err| HttpLikeError::Other(err.into()))?,
                data,
            )
            .await?)
    }
    async fn account_id_of_register_token(
        &self,
        data: Vec<u8>,
    ) -> anyhow::Result<Vec<u8>, HttpLikeError> {
        Ok(self
            .http
            .post_json(
                self.http
                    .base_url()
                    .join("/account-id-from-register-token")
                    .map_err(|err| HttpLikeError::Other(err.into()))?,
                data,
            )
            .await?)
    }
    async fn send_password_reset(&self, data: Vec<u8>) -> anyhow::Result<Vec<u8>, HttpLikeError> {
        Ok(self
            .http
            .post_json(
                self.http
                    .base_url()
                    .join("/password-reset")
                    .map_err(|err| HttpLikeError::Other(err.into()))?,
                data,
            )
            .await?)
    }
    async fn send_register(&self, data: Vec<u8>) -> anyhow::Result<Vec<u8>, HttpLikeError> {
        Ok(self
            .http
            .post_json(
                self.http
                    .base_url()
                    .join("/register")
                    .map_err(|err| HttpLikeError::Other(err.into()))?,
                data,
            )
            .await?)
    }
    async fn send_auth(&self, data: Vec<u8>) -> anyhow::Result<Vec<u8>, HttpLikeError> {
        Ok(self
            .http
            .post_json(
                self.http
                    .base_url()
                    .join("/auth")
                    .map_err(|err| HttpLikeError::Other(err.into()))?,
                data,
            )
            .await?)
    }
    async fn send_login(&self, data: Vec<u8>) -> anyhow::Result<Vec<u8>, HttpLikeError> {
        Ok(self
            .http
            .post_json(
                self.http
                    .base_url()
                    .join("/login")
                    .map_err(|err| HttpLikeError::Other(err.into()))?,
                data,
            )
            .await?)
    }
    async fn write_encrypted_main_secret_file(
        &self,
        file: Vec<u8>,
    ) -> anyhow::Result<(), FsLikeError> {
        self.fs.write("main_secret.key".as_ref(), file).await
    }
    async fn read_encrypted_main_secret_file(&self) -> anyhow::Result<Vec<u8>, FsLikeError> {
        self.fs.read("main_secret.key".as_ref()).await
    }
    async fn write_session_key_pair_file(&self, file: Vec<u8>) -> anyhow::Result<(), FsLikeError> {
        self.fs.write("session.key".as_ref(), file).await
    }
    async fn read_session_key_pair_file(&self) -> anyhow::Result<Vec<u8>, FsLikeError> {
        self.fs.read("session.key".as_ref()).await
    }
    async fn write_game_server_group_key_pair_file(
        &self,
        game_server_group_id: Option<GameServerGroupId>,
        file: Vec<u8>,
    ) -> anyhow::Result<(), FsLikeError> {
        if let Some(game_server_group_id) = game_server_group_id {
            let dir = hex::encode(game_server_group_id);
            self.fs.create_dirs(dir.as_ref()).await?;
            self.fs
                .write(format!("{}/pair.key", dir).as_ref(), file)
                .await
        } else {
            self.fs.write("pair.key".as_ref(), file).await
        }
    }
    async fn read_game_server_group_key_pair_file(
        &self,
        game_server_group_id: Option<GameServerGroupId>,
    ) -> anyhow::Result<Vec<u8>, FsLikeError> {
        if let Some(game_server_group_id) = game_server_group_id {
            self.fs
                .read(format!("{}/pair.key", hex::encode(game_server_group_id)).as_ref())
                .await
        } else {
            self.fs.read("pair.key".as_ref()).await
        }
    }
    async fn write_server_encrypted_main_secret_file(
        &self,
        file: Vec<u8>,
    ) -> anyhow::Result<(), FsLikeError> {
        self.fs
            .write("session_main_secret.key".as_ref(), file)
            .await
    }
    async fn read_server_encrypted_main_secret_file(&self) -> anyhow::Result<Vec<u8>, FsLikeError> {
        self.fs.read("session_main_secret.key".as_ref()).await
    }
}
