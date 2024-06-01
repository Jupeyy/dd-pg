pub mod queries;
pub mod rem_queries;

use std::sync::Arc;

use account_sql::query::Query;
use accounts_base::{
    account_server::{password_reset::PasswordResetResponse, register::RegisterResponse},
    client::password_reset::PasswordResetRequest,
};
use axum::{response, Json};
use base64::Engine;
use sqlx::{Acquire, MySqlPool};

use crate::{
    internal_err::InternalErr,
    password_reset::rem_queries::{
        GetAccountIdResetCode, RemClientKeys, RemGameServerKeys, RemSessions,
    },
    register::{prepare_register, register_prepare_session},
    shared::Shared,
};

use self::queries::VerifyResetCodeAndResetAccount;

pub async fn password_reset_request(
    shared: Arc<Shared>,
    pool: MySqlPool,
    Json(data): Json<PasswordResetRequest>,
) -> response::Result<Json<PasswordResetResponse>> {
    password_reset(shared, pool, data)
        .await
        .map_err(|err| InternalErr(("pw_reset".into(), err)).into())
        .map(Json)
}

pub async fn password_reset(
    shared: Arc<Shared>,
    pool: MySqlPool,
    data: PasswordResetRequest,
) -> anyhow::Result<PasswordResetResponse> {
    let (hash, salt) = prepare_register(&data.register_data)?;
    let reset_code = base64::prelude::BASE64_URL_SAFE.decode(data.reset_code_base64)?;

    let mut connection = pool.acquire().await?;
    let connection = connection.acquire().await?;

    // get account id for cleanup
    let qry = GetAccountIdResetCode {
        reset_code: &reset_code,
    };

    let qry_res = qry
        .query_mysql(&shared.mysql.get_account_id_from_reset_code_statement)
        .fetch_one(&mut *connection)
        .await?;

    let account_id = GetAccountIdResetCode::row_data(&qry_res)?;

    let qry = VerifyResetCodeAndResetAccount {
        reset_code: &reset_code,
        password_hash: hash,
        salt,
        serialized_main_secret: serde_json::to_string(&data.register_data.account_data.secret)?
            .into_bytes(),
    };

    let qry_res = qry
        .query_mysql(&shared.mysql.verify_reset_code_and_reset_account_statement)
        .execute(&mut *connection)
        .await?;
    anyhow::ensure!(
        qry_res.rows_affected() >= 1,
        "Failed to update account data on the database."
    );

    // Since resetting the password is similar to an account reset,
    // do some cleanup:
    // - try to clear all key-pairs related to this account
    // - try to clear all sessions
    // Intentionally ignore results.
    let qry = RemClientKeys {
        account_id: &account_id.account_id,
    };
    let _ = qry
        .query_mysql(&shared.mysql.clear_client_keys_statement)
        .execute(&mut *connection)
        .await;
    let qry = RemGameServerKeys {
        account_id: &account_id.account_id,
    };
    let _ = qry
        .query_mysql(&shared.mysql.clear_game_server_key_statement)
        .execute(&mut *connection)
        .await;
    let qry = RemSessions {
        account_id: &account_id.account_id,
    };
    let _ = qry
        .query_mysql(&shared.mysql.clear_sessions_statement)
        .execute(&mut *connection)
        .await;

    let auth_response = register_prepare_session(
        shared.clone(),
        pool.clone(),
        data.register_data.session_data,
    )
    .await;

    Ok(PasswordResetResponse {
        register_res: RegisterResponse::Success {
            requires_verification: false,
            auth_response,
        },
    })
}
