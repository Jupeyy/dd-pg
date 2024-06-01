pub mod queries;

use std::sync::Arc;

use account_sql::query::Query;
use accounts_base::{
    account_server::{
        auth::{AuthResponse, AuthResponseSecret, AuthResponseSuccess, AuthResponseVerified},
        secret::AccountServerSecret,
    },
    client::auth::AuthRequest,
};
use anyhow::anyhow;
use axum::{response, Json};
use sqlx::{Acquire, MySqlPool};

use crate::{internal_err::InternalErr, shared::Shared};

use self::queries::{AuthAttempt, AuthAttemptData};

pub async fn auth_request(
    shared: Arc<Shared>,
    pool: MySqlPool,
    Json(data): Json<AuthRequest>,
) -> response::Result<Json<AuthResponse>> {
    auth(shared, pool, data)
        .await
        .map_err(|err| InternalErr(("auth".into(), err)).into())
        .map(Json)
}

pub enum AuthVerifyResponse {
    Success(AuthAttemptData),
    NotFound,
}

pub async fn auth_verify(
    shared: Arc<Shared>,
    pool: MySqlPool,
    data: AuthRequest,
) -> anyhow::Result<AuthVerifyResponse> {
    anyhow::ensure!(
        shared.otps.try_consume_otp(data.otp),
        "One time password was not valid anymore (maybe the request was too slow)."
    );

    // first verify the signature
    data.pub_key
        .verify_strict(data.otp.as_slice(), &data.signature)?;

    let auth_attempt = AuthAttempt { data: &data };
    let mut connection = pool.acquire().await?;
    let row = match auth_attempt
        .query_mysql(&shared.mysql.auth_attempt_statement)
        .fetch_one(connection.acquire().await?)
        .await
    {
        Ok(row) => row,
        Err(err) => match err {
            sqlx::Error::RowNotFound => {
                // The auth attempt generally worked, but no matching session was found.
                return Ok(AuthVerifyResponse::NotFound);
            }
            _ => {
                return Err(anyhow!(err));
            }
        },
    };

    Ok(AuthVerifyResponse::Success(AuthAttempt::row_data(&row)?))
}

pub async fn auth(
    shared: Arc<Shared>,
    pool: MySqlPool,
    data: AuthRequest,
) -> anyhow::Result<AuthResponse> {
    let AuthVerifyResponse::Success(attempt_data) = auth_verify(shared, pool, data).await? else {
        return Ok(AuthResponse::Invalid);
    };

    let secret: AccountServerSecret =
        serde_json::from_str(&String::from_utf8(attempt_data.serialized_secret)?)?;

    let secret = AuthResponseSecret { secret };
    Ok(AuthResponse::Success(if attempt_data.verified {
        AuthResponseSuccess::Verified(AuthResponseVerified {
            secret,
            account_id: attempt_data.account_id,
        })
    } else {
        AuthResponseSuccess::NotVerified(secret)
    }))
}
