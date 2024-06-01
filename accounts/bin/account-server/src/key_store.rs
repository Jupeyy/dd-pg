pub mod game_server_queries;
pub mod queries;

use std::sync::Arc;

use account_sql::query::Query;
use accounts_base::{
    account_server::game_server_group::{
        GameServerKeyPairResponse, GameServerKeyPairResponseSuccess, StoreGameServerKeyPairResponse,
    },
    client::game_server_data::{
        GameServerKeyPair, RequestGameServerKeyPair, RequestStoreGameServerKeyPair,
    },
};

use axum::{response, Json};
use sqlx::{Acquire, MySqlPool};

use crate::{
    internal_err::InternalErr,
    key_store::{queries::GetGameServerGroupKeyPair, queries::StoreGameServerGroupKeyPair},
    shared::Shared,
};

pub async fn server_group_key_pair_request(
    shared: Arc<Shared>,
    pool: MySqlPool,
    Json(data): Json<RequestGameServerKeyPair>,
) -> response::Result<Json<GameServerKeyPairResponse>> {
    server_group_key_pair(shared, pool, data)
        .await
        .map_err(|err| InternalErr(("key_store".into(), err)).into())
        .map(Json)
}

pub async fn server_group_key_pair(
    shared: Arc<Shared>,
    pool: MySqlPool,
    data: RequestGameServerKeyPair,
) -> anyhow::Result<GameServerKeyPairResponse> {
    // verify otp
    anyhow::ensure!(
        shared.otps.try_consume_otp(data.auth_data.otp),
        "one time password was not valid anymore."
    );
    data.auth_data
        .pub_key
        .verify_strict(&data.auth_data.otp, &data.auth_data.signature)?;

    let mut connection = pool.acquire().await?;
    let connection = connection.acquire().await?;

    let key_pair: Option<Option<GameServerKeyPair>> =
        if let Some(game_server_group_id) = data.game_server_group_id {
            let qry = GetGameServerGroupKeyPair {
                game_server_group_id: &game_server_group_id,
                public_key: &data.auth_data.pub_key,
                hw_id: &data.auth_data.hw_id,
            };

            let qry_res = qry
                .query_mysql(&shared.mysql.get_game_server_group_key_pair_statement)
                .fetch_optional(&mut *connection)
                .await?;
            qry_res
                .map(|qry_res| {
                    let res = GetGameServerGroupKeyPair::row_data(&qry_res)?;
                    anyhow::Ok(
                        res.serialized_key_pair
                            .map(|serialized_key_pair| serde_json::from_slice(&serialized_key_pair))
                            .transpose()?,
                    )
                })
                .transpose()?
        } else {
            let qry = game_server_queries::GetGameServerGroupKeyPair {
                public_key: &data.auth_data.pub_key,
                hw_id: &data.auth_data.hw_id,
            };

            let qry_res = qry
                .query_mysql(
                    &shared
                        .mysql
                        .game_server_get_game_server_group_key_pair_statement,
                )
                .fetch_optional(&mut *connection)
                .await?;
            qry_res
                .map(|qry_res| {
                    let res = game_server_queries::GetGameServerGroupKeyPair::row_data(&qry_res)?;
                    anyhow::Ok(
                        res.serialized_key_pair
                            .map(|serialized_key_pair| serde_json::from_slice(&serialized_key_pair))
                            .transpose()?,
                    )
                })
                .transpose()?
        };

    key_pair.map_or_else(
        || Ok(GameServerKeyPairResponse::InvalidAuth),
        |key_pair| {
            key_pair.map_or_else(
                || Ok(GameServerKeyPairResponse::NotFound),
                |key_pair| {
                    Ok(GameServerKeyPairResponse::Success(Box::new(
                        GameServerKeyPairResponseSuccess { key_pair },
                    )))
                },
            )
        },
    )
}

pub async fn store_server_group_key_pair_request(
    shared: Arc<Shared>,
    pool: MySqlPool,
    Json(data): Json<RequestStoreGameServerKeyPair>,
) -> response::Result<Json<StoreGameServerKeyPairResponse>> {
    store_server_group_key_pair(shared, pool, data)
        .await
        .map_err(|err| InternalErr(("store_key_store".into(), err)).into())
        .map(Json)
}

pub async fn store_server_group_key_pair(
    shared: Arc<Shared>,
    pool: MySqlPool,
    data: RequestStoreGameServerKeyPair,
) -> anyhow::Result<StoreGameServerKeyPairResponse> {
    // verify otp
    anyhow::ensure!(
        shared.otps.try_consume_otp(data.auth_data.otp),
        "one time password was not valid anymore."
    );
    data.auth_data
        .pub_key
        .verify_strict(&data.auth_data.otp, &data.auth_data.signature)?;

    let mut connection = pool.acquire().await?;
    let connection = connection.acquire().await?;

    if let Some(game_server_group_id) = data.game_server_group_id {
        let qry = StoreGameServerGroupKeyPair {
            game_server_group_id: &game_server_group_id,
            serialized_key_pair: &serde_json::to_string(&data.key_pair.key_pair)?.into_bytes(),
            public_key: &data.auth_data.pub_key,
            hw_id: &data.auth_data.hw_id,
        };

        let qry_res = qry
            .query_mysql(&shared.mysql.store_game_server_group_key_pair_statement)
            .execute(&mut *connection)
            .await;
        match qry_res {
            Ok(qry_res) => {
                if qry_res.rows_affected() >= 1 {
                    Ok(StoreGameServerKeyPairResponse::Success)
                } else {
                    Ok(StoreGameServerKeyPairResponse::InvalidAuth)
                }
            }
            Err(err) => match err {
                sqlx::Error::Database(err) => match err.kind() {
                    sqlx::error::ErrorKind::ForeignKeyViolation
                    | sqlx::error::ErrorKind::NotNullViolation => {
                        Ok(StoreGameServerKeyPairResponse::GameServerGroupNotFound)
                    }
                    _ => Err(err.into()),
                },
                _ => Err(err.into()),
            },
        }
    } else {
        let qry = game_server_queries::StoreGameServerGroupKeyPair {
            add_serialized_key_pair: &serde_json::to_string(&data.key_pair.key_pair)?.into_bytes(),
            add_public: data.key_pair.key_pair.pub_key.as_bytes(),
            public_key: &data.auth_data.pub_key,
            hw_id: &data.auth_data.hw_id,
        };

        let qry_res = qry
            .query_mysql(
                &shared
                    .mysql
                    .game_server_store_game_server_group_key_pair_statement,
            )
            .execute(&mut *connection)
            .await?;
        if qry_res.rows_affected() >= 1 {
            Ok(StoreGameServerKeyPairResponse::Success)
        } else {
            Ok(StoreGameServerKeyPairResponse::InvalidAuth)
        }
    }
}
