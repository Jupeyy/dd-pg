use accounts_base::types::{EncryptedMainSecret, EncryptedMainSecretWithServerSecret};
use accounts_base::{
    account_server::{
        account_id::AccountId,
        auth::AuthResponse,
        game_server_group::{GameServerKeyPairResponse, StoreGameServerKeyPairResponse},
        login::LoginResponse,
        otp::OtpResponse,
        password_reset::PasswordResetResponse,
        register::RegisterResponse,
        register_token::{RegisterToken, RegisterTokenResponse},
    },
    client::{
        auth::AuthRequest,
        game_server_data::{
            GameServerKeyPair, RequestGameServerKeyPair, RequestStoreGameServerKeyPair,
        },
        otp::OtpRequest,
        password_forgot::PasswordForgotRequest,
        password_reset::PasswordResetRequest,
        register::RegisterDataForServer,
        reigster_token::RegisterTokenRequest,
        session::{SessionDataForClient, SessionDataForServer},
    },
    game_server::server_id::GameServerGroupId,
};
use anyhow::anyhow;
use async_trait::async_trait;
use ed25519_dalek::{SigningKey, VerifyingKey};
use serde::Deserialize;

use crate::{
    errors::{FsLikeError, HttpLikeError},
    interface::Io,
};

/// Type safe version of [`Io`]
#[async_trait]
pub trait SafeIo: Sync + Send {
    async fn request_otp(&self, data: OtpRequest) -> anyhow::Result<OtpResponse, HttpLikeError>;
    async fn request_register_token(
        &self,
        data: RegisterTokenRequest,
    ) -> anyhow::Result<RegisterTokenResponse, HttpLikeError>;
    async fn request_password_forgot(
        &self,
        data: PasswordForgotRequest,
    ) -> anyhow::Result<(), HttpLikeError>;
    async fn request_game_server_group_key_pair(
        &self,
        data: RequestGameServerKeyPair,
    ) -> anyhow::Result<GameServerKeyPairResponse, HttpLikeError>;
    async fn request_store_game_server_group_key_pair(
        &self,
        data: RequestStoreGameServerKeyPair,
    ) -> anyhow::Result<StoreGameServerKeyPairResponse, HttpLikeError>;
    async fn send_password_reset(
        &self,
        data: PasswordResetRequest,
    ) -> anyhow::Result<PasswordResetResponse, HttpLikeError>;
    async fn send_register(
        &self,
        data: RegisterDataForServer,
    ) -> anyhow::Result<RegisterResponse, HttpLikeError>;
    async fn send_auth(&self, data: AuthRequest) -> anyhow::Result<AuthResponse, HttpLikeError>;
    async fn send_login(
        &self,
        data: SessionDataForServer,
    ) -> anyhow::Result<LoginResponse, HttpLikeError>;
    async fn account_id_of_register_token(
        &self,
        data: RegisterToken,
    ) -> anyhow::Result<AccountId, HttpLikeError>;
    async fn write_encrypted_main_secret_file(
        &self,
        file: EncryptedMainSecret,
    ) -> anyhow::Result<(), FsLikeError>;
    async fn read_encrypted_main_secret_file(
        &self,
    ) -> anyhow::Result<EncryptedMainSecret, FsLikeError>;
    async fn write_game_server_group_key_pair_file(
        &self,
        game_server_group_id: Option<GameServerGroupId>,
        file: GameServerKeyPair,
    ) -> anyhow::Result<(), FsLikeError>;
    async fn read_game_server_group_key_pair_file(
        &self,
        game_server_group_id: Option<GameServerGroupId>,
    ) -> anyhow::Result<GameServerKeyPair, FsLikeError>;
    async fn write_server_encrypted_main_secret_file(
        &self,
        file: EncryptedMainSecretWithServerSecret,
    ) -> anyhow::Result<(), FsLikeError>;
    async fn read_server_encrypted_main_secret_file(
        &self,
    ) -> anyhow::Result<EncryptedMainSecretWithServerSecret, FsLikeError>;
    async fn write_session_key_pair_file(
        &self,
        file: SessionDataForClient,
    ) -> anyhow::Result<(), FsLikeError>;
    async fn read_session_key_pair_file(&self)
        -> anyhow::Result<SessionDataForClient, FsLikeError>;
}

pub struct IoSafe<'a> {
    pub io: &'a dyn Io,
}

impl<'a> IoSafe<'a> {
    fn des_from_vec<T>(data: Vec<u8>) -> anyhow::Result<T, HttpLikeError>
    where
        for<'de> T: Deserialize<'de>,
    {
        let s = String::from_utf8(data).map_err(|err| HttpLikeError::Other(err.into()))?;
        serde_json::from_str(s.as_str())
            .map_err(|_| HttpLikeError::Other(anyhow!("failed to parse json: {s}")))
    }
}

impl<'a> From<&'a dyn Io> for IoSafe<'a> {
    fn from(io: &'a dyn Io) -> Self {
        Self { io }
    }
}

#[async_trait]
impl<'a> SafeIo for IoSafe<'a> {
    async fn request_otp(&self, data: OtpRequest) -> anyhow::Result<OtpResponse, HttpLikeError> {
        let otp = self
            .io
            .request_otp(serde_json::to_string(&data)?.into_bytes())
            .await?;
        Self::des_from_vec(otp)
    }
    async fn request_register_token(
        &self,
        data: RegisterTokenRequest,
    ) -> anyhow::Result<RegisterTokenResponse, HttpLikeError> {
        let res = self
            .io
            .request_register_token(serde_json::to_string(&data)?.into_bytes())
            .await?;
        Self::des_from_vec(res)
    }
    async fn request_password_forgot(
        &self,
        data: PasswordForgotRequest,
    ) -> anyhow::Result<(), HttpLikeError> {
        let res = self
            .io
            .request_password_forgot(serde_json::to_string(&data)?.into_bytes())
            .await?;
        Self::des_from_vec(res)
    }
    async fn request_game_server_group_key_pair(
        &self,
        data: RequestGameServerKeyPair,
    ) -> anyhow::Result<GameServerKeyPairResponse, HttpLikeError> {
        let res = self
            .io
            .request_game_server_group_key_pair(serde_json::to_string(&data)?.into_bytes())
            .await?;
        Self::des_from_vec(res)
    }
    async fn request_store_game_server_group_key_pair(
        &self,
        data: RequestStoreGameServerKeyPair,
    ) -> anyhow::Result<StoreGameServerKeyPairResponse, HttpLikeError> {
        let res = self
            .io
            .request_store_game_server_group_key_pair(serde_json::to_string(&data)?.into_bytes())
            .await?;
        Self::des_from_vec(res)
    }
    async fn send_password_reset(
        &self,
        data: PasswordResetRequest,
    ) -> anyhow::Result<PasswordResetResponse, HttpLikeError> {
        let res = self
            .io
            .send_password_reset(serde_json::to_string(&data)?.into_bytes())
            .await?;
        Self::des_from_vec(res)
    }
    async fn send_register(
        &self,
        data: RegisterDataForServer,
    ) -> anyhow::Result<RegisterResponse, HttpLikeError> {
        let res = self
            .io
            .send_register(serde_json::to_string(&data)?.into_bytes())
            .await?;
        Self::des_from_vec(res)
    }
    async fn send_auth(&self, msg: AuthRequest) -> anyhow::Result<AuthResponse, HttpLikeError> {
        let res = self
            .io
            .send_auth(serde_json::to_string(&msg)?.into_bytes())
            .await?;
        Self::des_from_vec(res)
    }
    async fn send_login(
        &self,
        msg: SessionDataForServer,
    ) -> anyhow::Result<LoginResponse, HttpLikeError> {
        let res = self
            .io
            .send_login(serde_json::to_string(&msg)?.into_bytes())
            .await?;
        Self::des_from_vec(res)
    }
    async fn account_id_of_register_token(
        &self,
        data: RegisterToken,
    ) -> anyhow::Result<AccountId, HttpLikeError> {
        let res = self
            .io
            .account_id_of_register_token(serde_json::to_string(&data)?.into_bytes())
            .await?;
        Self::des_from_vec(res)
    }
    async fn write_encrypted_main_secret_file(
        &self,
        file: EncryptedMainSecret,
    ) -> anyhow::Result<(), FsLikeError> {
        self.io
            .write_encrypted_main_secret_file(
                serde_json::to_string(&file)
                    .map_err(|err| FsLikeError::Other(err.into()))?
                    .into_bytes(),
            )
            .await
    }
    async fn read_encrypted_main_secret_file(
        &self,
    ) -> anyhow::Result<EncryptedMainSecret, FsLikeError> {
        Ok(serde_json::from_str(
            String::from_utf8(self.io.read_encrypted_main_secret_file().await?)
                .map_err(|err| FsLikeError::Other(err.into()))?
                .as_str(),
        )
        .map_err(|err| FsLikeError::Other(err.into()))?)
    }
    async fn write_game_server_group_key_pair_file(
        &self,
        game_server_group_id: Option<GameServerGroupId>,
        file: GameServerKeyPair,
    ) -> anyhow::Result<(), FsLikeError> {
        self.io
            .write_game_server_group_key_pair_file(
                game_server_group_id,
                serde_json::to_string(&file)
                    .map_err(|err| FsLikeError::Other(err.into()))?
                    .into_bytes(),
            )
            .await
    }
    async fn read_game_server_group_key_pair_file(
        &self,
        game_server_group_id: Option<GameServerGroupId>,
    ) -> anyhow::Result<GameServerKeyPair, FsLikeError> {
        Ok(serde_json::from_str(
            String::from_utf8(
                self.io
                    .read_game_server_group_key_pair_file(game_server_group_id)
                    .await?,
            )
            .map_err(|err| FsLikeError::Other(err.into()))?
            .as_str(),
        )
        .map_err(|err| FsLikeError::Other(err.into()))?)
    }
    async fn write_server_encrypted_main_secret_file(
        &self,
        file: EncryptedMainSecretWithServerSecret,
    ) -> anyhow::Result<(), FsLikeError> {
        self.io
            .write_server_encrypted_main_secret_file(
                serde_json::to_string(&file)
                    .map_err(|err| FsLikeError::Other(err.into()))?
                    .into_bytes(),
            )
            .await
    }
    async fn read_server_encrypted_main_secret_file(
        &self,
    ) -> anyhow::Result<EncryptedMainSecretWithServerSecret, FsLikeError> {
        Ok(serde_json::from_str(
            String::from_utf8(self.io.read_server_encrypted_main_secret_file().await?)
                .map_err(|err| FsLikeError::Other(err.into()))?
                .as_str(),
        )
        .map_err(|err| FsLikeError::Other(err.into()))?)
    }
    async fn write_session_key_pair_file(
        &self,
        file: SessionDataForClient,
    ) -> anyhow::Result<(), FsLikeError> {
        self.io
            .write_session_key_pair_file(
                serde_json::to_string(&(file.priv_key, file.pub_key))
                    .map_err(|err| FsLikeError::Other(err.into()))?
                    .into_bytes(),
            )
            .await
    }
    async fn read_session_key_pair_file(
        &self,
    ) -> anyhow::Result<SessionDataForClient, FsLikeError> {
        let (priv_key, pub_key): (SigningKey, VerifyingKey) = serde_json::from_str(
            String::from_utf8(self.io.read_session_key_pair_file().await?)
                .map_err(|err| FsLikeError::Other(err.into()))?
                .as_str(),
        )
        .map_err(|err| FsLikeError::Other(err.into()))?;
        Ok(SessionDataForClient { priv_key, pub_key })
    }
}
