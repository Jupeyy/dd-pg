pub mod queries;

use std::{str::FromStr, sync::Arc};

use account_sql::query::Query;
use accounts_shared::client::login::LoginRequest;
use axum::{response, Json};
use sqlx::{Acquire, AnyPool, Connection};

use crate::{internal_err::InternalErr, shared::Shared};

use self::queries::{
    CreateSession, InvalidateLoginToken, LoginQry, LoginTokenQry, TryCreateAccount,
};

pub async fn login_request(
    shared: Arc<Shared>,
    pool: AnyPool,
    Json(data): Json<LoginRequest>,
) -> response::Result<Json<()>> {
    login(shared, pool, data)
        .await
        .map_err(|err| InternalErr(("login".into(), err)).into())
        .map(Json)
}

pub async fn login(shared: Arc<Shared>, pool: AnyPool, data: LoginRequest) -> anyhow::Result<()> {
    // first verify the signature
    // this step isn't really needed (security wise),
    // but at least proofs the client has a valid private key.
    data.account_data
        .public_key
        .verify_strict(data.login_token.as_slice(), &data.login_token_signature)?;

    let mut connection = pool.acquire().await?;
    let connection = connection.acquire().await?;

    connection
        .transaction(|connection| {
            Box::pin(async move {
                // token data
                let login_token_qry = LoginTokenQry {
                    token: &data.login_token,
                };

                let row = login_token_qry
                    .query(&shared.db.login_token_qry_statement)
                    .fetch_one(&mut **connection)
                    .await?;

                let token_data = LoginTokenQry::row_data(&row)?;

                // invalidate token
                let qry = InvalidateLoginToken {
                    token: &data.login_token,
                };
                qry.query(&shared.db.invalidate_login_token_statement)
                    .execute(&mut **connection)
                    .await?;

                let email = token_data
                    .email
                    .map(|email| email_address::EmailAddress::from_str(&email))
                    .transpose()?;

                // create account (if not exists)
                let qry = TryCreateAccount {
                    email: &email,
                    steam_id: &token_data.steam_id,
                };

                let res = qry
                    .query(&shared.db.try_create_account_statement)
                    .execute(&mut **connection)
                    .await;

                // ignore unique violations etc.
                if let Err(err) = res {
                    match err {
                        sqlx::Error::Database(_) => Ok(()),
                        _ => Err(err),
                    }?;
                }

                // query account data
                let login_qry = LoginQry {
                    email: &email,
                    steam_id: &None,
                };

                let row = login_qry
                    .query(&shared.db.login_qry_statement)
                    .fetch_one(&mut **connection)
                    .await?;

                let login_data = LoginQry::row_data(&row)?;

                let qry = CreateSession {
                    account_id: login_data.account_id,
                    hw_id: &data.account_data.hw_id,
                    pub_key: data.account_data.public_key.as_bytes(),
                };

                qry.query(&shared.db.create_session_statement)
                    .execute(&mut **connection)
                    .await?;

                anyhow::Ok(())
            })
        })
        .await?;

    Ok(())
}
