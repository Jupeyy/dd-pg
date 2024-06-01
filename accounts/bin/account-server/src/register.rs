pub mod queries;

use std::sync::Arc;

use account_sql::query::Query;
use accounts_base::{
    account_server::{
        auth::AuthResponse, login::LoginResponse, otp::generate_otp, register::RegisterResponse,
    },
    client::{
        password::hash_bytes, register::RegisterDataForServer, session::SessionDataForServer,
    },
};
use anyhow::anyhow;
use argon2::password_hash::SaltString;
use axum::{response, Json};
use base64::Engine;
use sqlx::{Acquire, MySqlPool};

use crate::{
    internal_err::InternalErr, register::queries::AddVerifyToken, session::create_session,
    shared::Shared,
};

use self::queries::AddAccount;

pub async fn register(
    shared: Arc<Shared>,
    pool: MySqlPool,
    Json(register_data): Json<RegisterDataForServer>,
) -> response::Result<Json<RegisterResponse>> {
    register_impl(shared, pool, register_data)
        .await
        .map_err(|err| InternalErr(("register".into(), err)).into())
}

pub fn prepare_register(
    register_data: &RegisterDataForServer,
) -> anyhow::Result<([u8; 32], SaltString)> {
    // hash password again
    let hashed_password = hash_bytes(&register_data.account_data.hashed_password)?;

    let salt = SaltString::from_b64(&hashed_password.salt).map_err(|err| anyhow!(err))?;

    Ok((hashed_password.hash, salt))
}

pub async fn register_prepare_session(
    shared: Arc<Shared>,
    pool: MySqlPool,
    session_data: SessionDataForServer,
) -> AuthResponse {
    (create_session(shared, pool, session_data).await).map_or(AuthResponse::Invalid, |login_res| {
        match login_res {
            LoginResponse::Success(res) => AuthResponse::Success(res.auth),
            LoginResponse::InvalidPasswordOrEmail => AuthResponse::Invalid,
        }
    })
}

pub async fn register_impl(
    shared: Arc<Shared>,
    pool: MySqlPool,
    register_data: RegisterDataForServer,
) -> anyhow::Result<Json<RegisterResponse>> {
    let (hash, salt) = prepare_register(&register_data)?;

    // write the new account to the database
    // Add a verify token and send it by email
    let token = generate_otp();
    let token_base_64 = base64::prelude::BASE64_URL_SAFE.encode(token);
    let query = AddAccount {
        data: &register_data,
        password_hash: hash,
        salt,
        serialized_main_secret: serde_json::to_string(&register_data.account_data.secret)?
            .into_bytes(),
    };
    let query_add_verify = AddVerifyToken {
        verify_token: &token,
    };
    let mut connection = pool.acquire().await?;
    let con = connection.acquire().await?;
    let query_res = query
        .query_mysql(&shared.mysql.register_statement)
        .execute(&mut *con)
        .await;

    if let Err(err) = query_res {
        match err {
            sqlx::Error::Database(err) => match err.kind() {
                sqlx::error::ErrorKind::UniqueViolation => {
                    return Ok(Json(RegisterResponse::AccountWithEmailAlreadyExists));
                }
                _ => return Err(err.into()),
            },
            _ => return Err(err.into()),
        }
    }
    let query_res = query_res?;
    if query_res.rows_affected() < 1 {
        return Ok(Json(RegisterResponse::AccountWithEmailAlreadyExists));
    }

    let verify_token_res = query_add_verify
        .query_mysql(&shared.mysql.add_verify_token)
        .execute(&mut *con)
        .await;
    if let Err(err) = verify_token_res {
        log::info!(target: "register", "registering failed unexpected: {}", err);
    }

    // At this moment the account exists (on the database).
    // If the session fails to be created that is not a critical error anymore.
    // So match all errors explicitly.
    let auth_response =
        register_prepare_session(shared.clone(), pool.clone(), register_data.session_data).await;

    // If step fails it's still not a fatal error,
    // since this token can be regenerated and simply be resent
    let send_email = async {
        shared
            .email
            .send_email(
                register_data.email.as_str(),
                "DDNet Account Registration",
                format!(
                    "Hello {},\nTo finish the registration of your account \
                    please open this link:\n<a href='{}'></a>",
                    register_data.email.local_part(),
                    shared
                        .http_url
                        .join(&format!("complete-register?token={}", token_base_64))?
                ),
            )
            .await?;
        anyhow::Ok(())
    };
    let email_res = send_email.await;
    if let Err(err) = email_res {
        log::info!(target: "register", "sending verifying email failed unexpected: {}", err);
    }

    Ok(Json(RegisterResponse::Success {
        requires_verification: true,
        auth_response,
    }))
}
