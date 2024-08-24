use accounts_shared::{
    account_server::{login::LoginError, result::AccountServerReqResult, sign::SignResponse},
    client::{
        account_data::AccountDataForClient,
        account_token::AccountTokenEmailRequest,
        delete::{DeleteRequest, DeleteSessionsRequest},
        login::LoginRequest,
        login_token_email::LoginTokenEmailRequest,
        logout::LogoutRequest,
        sign::SignRequest,
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
    ) -> anyhow::Result<AccountServerReqResult<(), ()>, HttpLikeError>;
    async fn request_login(
        &self,
        data: LoginRequest,
    ) -> anyhow::Result<AccountServerReqResult<(), LoginError>, HttpLikeError>;
    async fn request_logout(&self, data: LogoutRequest) -> anyhow::Result<(), HttpLikeError>;
    async fn request_sign(&self, data: SignRequest) -> anyhow::Result<SignResponse, HttpLikeError>;
    async fn request_account_token_email(
        &self,
        data: AccountTokenEmailRequest,
    ) -> anyhow::Result<(), HttpLikeError>;
    async fn request_delete_sessions(
        &self,
        data: DeleteSessionsRequest,
    ) -> anyhow::Result<(), HttpLikeError>;
    async fn request_delete_account(
        &self,
        data: DeleteRequest,
    ) -> anyhow::Result<(), HttpLikeError>;
    async fn download_account_server_certificates(
        &self,
    ) -> anyhow::Result<Vec<Vec<u8>>, HttpLikeError>;
    async fn write_serialized_session_key_pair(
        &self,
        file: &AccountDataForClient,
    ) -> anyhow::Result<(), FsLikeError>;
    async fn read_serialized_session_key_pair(
        &self,
    ) -> anyhow::Result<AccountDataForClient, FsLikeError>;
    async fn remove_serialized_session_key_pair(&self) -> anyhow::Result<(), FsLikeError>;
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
    ) -> anyhow::Result<AccountServerReqResult<(), ()>, HttpLikeError> {
        let res = self
            .io
            .request_login_email_token(serde_json::to_string(&data)?.into_bytes())
            .await?;
        Self::des_from_vec(res)
    }
    async fn request_login(
        &self,
        data: LoginRequest,
    ) -> anyhow::Result<AccountServerReqResult<(), LoginError>, HttpLikeError> {
        let res = self
            .io
            .request_login(serde_json::to_string(&data)?.into_bytes())
            .await?;
        Self::des_from_vec(res)
    }
    async fn request_logout(&self, data: LogoutRequest) -> anyhow::Result<(), HttpLikeError> {
        self.io
            .request_logout(serde_json::to_string(&data)?.into_bytes())
            .await
    }
    async fn request_sign(&self, data: SignRequest) -> anyhow::Result<SignResponse, HttpLikeError> {
        let res = self
            .io
            .request_sign(serde_json::to_string(&data)?.into_bytes())
            .await?;
        Self::des_from_vec(res)
    }
    async fn request_account_token_email(
        &self,
        data: AccountTokenEmailRequest,
    ) -> anyhow::Result<(), HttpLikeError> {
        self.io
            .request_account_token_email(serde_json::to_string(&data)?.into_bytes())
            .await
    }
    async fn request_delete_sessions(
        &self,
        data: DeleteSessionsRequest,
    ) -> anyhow::Result<(), HttpLikeError> {
        self.io
            .request_delete_sessions(serde_json::to_string(&data)?.into_bytes())
            .await
    }
    async fn request_delete_account(
        &self,
        data: DeleteRequest,
    ) -> anyhow::Result<(), HttpLikeError> {
        self.io
            .request_delete_account(serde_json::to_string(&data)?.into_bytes())
            .await
    }
    async fn download_account_server_certificates(
        &self,
    ) -> anyhow::Result<Vec<Vec<u8>>, HttpLikeError> {
        let res = self.io.download_account_server_certificates().await?;

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
        Ok(
            serde_json::from_slice(&self.io.read_serialized_session_key_pair().await?)
                .map_err(|err| FsLikeError::Other(err.into()))?,
        )
    }
    async fn remove_serialized_session_key_pair(&self) -> anyhow::Result<(), FsLikeError> {
        self.io.remove_serialized_session_key_pair().await
    }
}
