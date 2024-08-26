pub mod queries;

use std::{str::FromStr, sync::Arc};

use account_sql::query::Query;
use accounts_shared::{
    account_server::{
        errors::AccountServerRequestError, login::LoginError, result::AccountServerReqResult,
    },
    client::login::LoginRequest,
};
use axum::Json;
use queries::{
    AccountIdFromEmail, AccountIdFromLastInsert, AccountIdFromSteam, LinkAccountCredentialEmail,
    LinkAccountCredentialSteam,
};
use sqlx::{Acquire, AnyPool, Connection};

use crate::{shared::Shared, types::TokenType};

use self::queries::{CreateSession, InvalidateLoginToken, LoginTokenQry, TryCreateAccount};

pub async fn login_request(
    shared: Arc<Shared>,
    pool: AnyPool,
    Json(data): Json<LoginRequest>,
) -> Json<AccountServerReqResult<(), LoginError>> {
    Json(login(shared, pool, data).await)
}

#[derive(Debug, Clone)]
enum LoginResponse {
    /// Worked
    Success,
    /// Token invalid, probably timed out
    TokenInvalid,
}

pub async fn login(
    shared: Arc<Shared>,
    pool: AnyPool,
    data: LoginRequest,
) -> AccountServerReqResult<(), LoginError> {
    let res = async {
        // first verify the signature
        // this step isn't really needed (security wise),
        // but at least proofs the client has a valid private key.
        data.account_data
            .public_key
            .verify_strict(data.login_token.as_slice(), &data.login_token_signature)?;

        let mut connection = pool.acquire().await?;
        let connection = connection.acquire().await?;

        let res = connection
            .transaction(|connection| {
                Box::pin(async move {
                    // token data
                    let login_token_qry = LoginTokenQry {
                        token: &data.login_token,
                    };

                    let row = login_token_qry
                        .query(&shared.db.login_token_qry_statement)
                        .fetch_optional(&mut **connection)
                        .await?;

                    let row = match row {
                        Some(row) => row,
                        None => return Ok(LoginResponse::TokenInvalid),
                    };

                    let token_data = LoginTokenQry::row_data(&row)?;

                    // invalidate token
                    let qry = InvalidateLoginToken {
                        token: &data.login_token,
                    };
                    qry.query(&shared.db.invalidate_login_token_statement)
                        .execute(&mut **connection)
                        .await?;

                    // create account (if not exists)
                    let account_id = match token_data.ty {
                        TokenType::Email => {
                            let email =
                                email_address::EmailAddress::from_str(&token_data.identifier)?;
                            // query account data
                            let qry = AccountIdFromEmail { email: &email };

                            let row = qry
                                .query(&shared.db.account_id_from_email_qry_statement)
                                .fetch_optional(&mut **connection)
                                .await?;

                            row.map(|row| AccountIdFromEmail::row_data(&row))
                                .transpose()?
                                .map(|data| data.account_id)
                        }
                        TokenType::Steam => {
                            let steamid64: i64 = token_data.identifier.parse()?;
                            // query account data
                            let qry = AccountIdFromSteam {
                                steamid64: &steamid64,
                            };

                            let row = qry
                                .query(&shared.db.account_id_from_steam_qry_statement)
                                .fetch_optional(&mut **connection)
                                .await?;

                            row.map(|row| AccountIdFromSteam::row_data(&row))
                                .transpose()?
                                .map(|data| data.account_id)
                        }
                    };

                    let account_id = match account_id {
                        Some(account_id) => account_id,
                        None => {
                            let qry = TryCreateAccount {};

                            let res = qry
                                .query(&shared.db.try_create_account_statement)
                                .execute(&mut **connection)
                                .await?;

                            anyhow::ensure!(res.rows_affected() >= 1, "account was not created");

                            // query account data
                            let login_qry = AccountIdFromLastInsert {};
                            let row = login_qry
                                .query(&shared.db.account_id_from_last_insert_qry_statement)
                                .fetch_one(&mut **connection)
                                .await?;

                            let login_data = AccountIdFromLastInsert::row_data(&row)?;

                            match token_data.ty {
                                TokenType::Email => {
                                    let email = email_address::EmailAddress::from_str(
                                        &token_data.identifier,
                                    )?;
                                    let qry = LinkAccountCredentialEmail {
                                        account_id: &login_data.account_id,
                                        email: &email,
                                    };

                                    let res = qry
                                        .query(
                                            &shared.db.link_credentials_email_login_qry_statement,
                                        )
                                        .execute(&mut **connection)
                                        .await?;

                                    anyhow::ensure!(
                                        res.rows_affected() >= 1,
                                        "account was not created, linking email failed"
                                    );
                                }
                                TokenType::Steam => {
                                    let steamid64: i64 = token_data.identifier.parse()?;
                                    let qry = LinkAccountCredentialSteam {
                                        account_id: &login_data.account_id,
                                        steamid64: &steamid64,
                                    };

                                    let res = qry
                                        .query(
                                            &shared.db.link_credentials_steam_login_qry_statement,
                                        )
                                        .execute(&mut **connection)
                                        .await?;

                                    anyhow::ensure!(
                                        res.rows_affected() >= 1,
                                        "account was not created, linking steam failed"
                                    );
                                }
                            }
                            login_data.account_id
                        }
                    };

                    let qry = CreateSession {
                        account_id,
                        hw_id: &data.account_data.hw_id,
                        pub_key: data.account_data.public_key.as_bytes(),
                    };

                    qry.query(&shared.db.create_session_statement)
                        .execute(&mut **connection)
                        .await?;

                    anyhow::Ok(LoginResponse::Success)
                })
            })
            .await?;
        anyhow::Ok(res)
    }
    .await
    .map_err(|err| AccountServerRequestError::Unexpected {
        target: "login".into(),
        err: err.to_string(),
        bt: err.backtrace().to_string(),
    })?;

    match res {
        LoginResponse::Success => Ok(()),
        LoginResponse::TokenInvalid => Err(AccountServerRequestError::LogicError(
            LoginError::TokenInvalid,
        )),
    }
}
