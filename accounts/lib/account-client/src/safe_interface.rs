use accounts_base::{
    account_server::sign::SignResponse,
    client::{
        account_data::AccountDataForClient, login::LoginRequest,
        login_token_email::LoginTokenEmailRequest, sign::SignRequest,
    },
};
use anyhow::anyhow;
use async_trait::async_trait;
use serde::Deserialize;

use crate::{
    errors::{FsLikeError, HttpLikeError},
    interface::Io,
};

/// Type safe version of [`Io`]
#[async_trait]
pub trait SafeIo: Sync + Send {
    async fn request_login_email_token(
        &self,
        data: LoginTokenEmailRequest,
    ) -> anyhow::Result<(), HttpLikeError>;
    async fn request_login(&self, data: LoginRequest) -> anyhow::Result<(), HttpLikeError>;
    async fn request_sign(&self, data: SignRequest) -> anyhow::Result<SignResponse, HttpLikeError>;
    async fn write_serialized_session_key_pair(
        &self,
        file: &AccountDataForClient,
    ) -> anyhow::Result<(), FsLikeError>;
    async fn read_serialized_session_key_pair(
        &self,
    ) -> anyhow::Result<AccountDataForClient, FsLikeError>;
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
    async fn request_login_email_token(
        &self,
        data: LoginTokenEmailRequest,
    ) -> anyhow::Result<(), HttpLikeError> {
        let res = self
            .io
            .request_login_email_token(serde_json::to_string(&data)?.into_bytes())
            .await?;
        Self::des_from_vec(res)
    }
    async fn request_login(&self, data: LoginRequest) -> anyhow::Result<(), HttpLikeError> {
        let res = self
            .io
            .request_login(serde_json::to_string(&data)?.into_bytes())
            .await?;
        Self::des_from_vec(res)
    }
    async fn request_sign(&self, data: SignRequest) -> anyhow::Result<SignResponse, HttpLikeError> {
        let res = self
            .io
            .request_sign(serde_json::to_string(&data)?.into_bytes())
            .await?;
        Self::des_from_vec(res)
    }
    async fn write_serialized_session_key_pair(
        &self,
        file: &AccountDataForClient,
    ) -> anyhow::Result<(), FsLikeError> {
        self.io
            .write_serialized_session_key_pair(
                serde_json::to_string(file)
                    .map_err(|err| FsLikeError::Other(err.into()))?
                    .into_bytes(),
            )
            .await
    }
    async fn read_serialized_session_key_pair(
        &self,
    ) -> anyhow::Result<AccountDataForClient, FsLikeError> {
        Ok(serde_json::from_str(
            String::from_utf8(self.io.read_serialized_session_key_pair().await?)
                .map_err(|err| FsLikeError::Other(err.into()))?
                .as_str(),
        )
        .map_err(|err| FsLikeError::Other(err.into()))?)
    }
}
