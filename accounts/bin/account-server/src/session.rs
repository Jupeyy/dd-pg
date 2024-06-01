pub mod queries;

use std::sync::Arc;

use account_sql::query::Query;
use accounts_base::types::EncryptedMainSecret;
use accounts_base::{
    account_server::{
        auth::AuthResponse,
        login::{LoginResponse, LoginResponseSuccess},
        secret::generate_account_server_secret,
    },
    client::{password::argon2_hash_from_salt, session::SessionDataForServer},
};
use anyhow::anyhow;
use axum::{response, Json};
use sqlx::{Acquire, MySqlPool};

use crate::{auth::auth, internal_err::InternalErr, shared::Shared};

use self::queries::{CreateSession, LoginAttempt};

pub async fn create_session_request(
    shared: Arc<Shared>,
    pool: MySqlPool,
    Json(data): Json<SessionDataForServer>,
) -> response::Result<Json<LoginResponse>> {
    create_session(shared, pool, data)
        .await
        .map_err(|err| InternalErr(("session".into(), err)).into())
        .map(Json)
}

pub async fn create_session(
    shared: Arc<Shared>,
    pool: MySqlPool,
    data: SessionDataForServer,
) -> anyhow::Result<LoginResponse> {
    anyhow::ensure!(
        shared.otps.try_consume_otp(data.otp),
        "One time password was not valid anymore (maybe the request was too slow)."
    );

    // first verify the signature
    data.pub_key
        .verify_strict(data.otp.as_slice(), &data.signature)?;

    let login_attempt = LoginAttempt { data: &data };

    let mut connection = pool.acquire().await?;
    let row = match login_attempt
        .query_mysql(&shared.mysql.login_attempt_statement)
        .fetch_one(connection.acquire().await?)
        .await
    {
        Ok(row) => row,
        Err(err) => {
            match err {
                sqlx::Error::RowNotFound => {
                    // if no row found the request generally worked, but it was an invalid email
                    return Ok(LoginResponse::InvalidPasswordOrEmail);
                }
                _ => return Err(anyhow!(err)),
            }
        }
    };

    let attempt_data = LoginAttempt::row_data(&row)?;

    let hashed_password =
        argon2_hash_from_salt(&data.hashed_password, attempt_data.salt.as_salt())?;

    if hashed_password == attempt_data.password {
        // parse encrypted main secret, the client will use it to rewrite the secrets's encryption
        // using the server provided secret
        let encrypted_main_secret: EncryptedMainSecret = serde_json::from_str(&String::from_utf8(
            attempt_data.serialized_encrypted_main_secret,
        )?)?;

        let secret = generate_account_server_secret();

        // now that the login worked, create a session and process the auth
        let session = CreateSession {
            account_id: attempt_data.account_id,
            pub_key: data.pub_key.as_bytes(),
            serialized_secret: serde_json::to_string(&secret)?.into_bytes(),
            hw_id: &data.hw_id,
        };
        let query_res = session
            .query_mysql(&shared.mysql.create_session_statement)
            .execute(connection.acquire().await?)
            .await?;
        anyhow::ensure!(
            query_res.rows_affected() >= 1,
            "Session was not created by the last query."
        );

        // now that the session exist, do the auth
        let AuthResponse::Success(auth_res) = auth(shared, pool, data.auth_request).await? else {
            return Err(anyhow!(
                "Auth failed unexpected directly after creating the session"
            ));
        };

        Ok(LoginResponse::Success(LoginResponseSuccess {
            auth: auth_res,
            main_secret: encrypted_main_secret,
        }))
    } else {
        Ok(LoginResponse::InvalidPasswordOrEmail)
    }
}
