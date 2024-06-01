use std::sync::Arc;

use accounts_base::{
    account_server::{
        account_id::AccountId,
        register_token::{RegisterToken, RegisterTokenResponse},
    },
    client::reigster_token::RegisterTokenRequest,
};
use anyhow::anyhow;
use axum::{response, Json};
use sqlx::MySqlPool;

use crate::{
    auth::{auth_verify, AuthVerifyResponse},
    internal_err::InternalErr,
    shared::Shared,
};

pub async fn register_token_request(
    shared: Arc<Shared>,
    pool: MySqlPool,
    Json(data): Json<RegisterTokenRequest>,
) -> response::Result<Json<RegisterTokenResponse>> {
    register_token_impl(shared, pool, data)
        .await
        .map_err(|err| InternalErr(("register_token".into(), err)).into())
        .map(Json)
}

async fn register_token_impl(
    shared: Arc<Shared>,
    pool: MySqlPool,
    data: RegisterTokenRequest,
) -> anyhow::Result<RegisterTokenResponse> {
    let AuthVerifyResponse::Success(auth_data) =
        auth_verify(shared.clone(), pool, data.auth_req).await?
    else {
        return Err(anyhow!("User was not authed."));
    };

    anyhow::ensure!(auth_data.verified, "User was not yet verified.");

    let token = shared
        .register_tokens
        .gen_register_token_for(auth_data.account_id);

    Ok(RegisterTokenResponse { token })
}

pub fn account_id_from_register_token_request(
    shared: Arc<Shared>,

    Json(data): Json<RegisterToken>,
) -> response::Result<Json<AccountId>> {
    account_id_from_register_token_request_impl(shared, data)
        .map_err(|err| InternalErr(("account_id_from_register_token".into(), err)).into())
        .map(Json)
}

pub fn account_id_from_register_token_request_impl(
    shared: Arc<Shared>,

    data: RegisterToken,
) -> anyhow::Result<AccountId> {
    shared
        .register_tokens
        .try_consume_register_token(data)
        .ok_or_else(|| anyhow!("no such register token was found."))
}
