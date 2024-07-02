pub mod queries;

use std::sync::Arc;

use account_sql::query::Query;
use axum::{extract, response};
use base64::Engine;
use serde::Deserialize;
use sqlx::{Acquire, MySqlPool};

use crate::{complete_register::queries::VerifyAccount, internal_err::InternalErr, shared::Shared};

#[derive(Debug, Deserialize)]
pub struct CompleteRegisterToken {
    token: String,
}

pub async fn complete_register(
    shared: Arc<Shared>,
    pool: MySqlPool,
    extract::Query(register_data): extract::Query<CompleteRegisterToken>,
) -> response::Result<()> {
    complete_register_impl(shared, pool, register_data)
        .await
        .map_err(|err| InternalErr(("complete_register".into(), err)).into())
}

pub async fn complete_register_impl(
    shared: Arc<Shared>,
    pool: MySqlPool,
    register_data: CompleteRegisterToken,
) -> anyhow::Result<()> {
    // try to verify the account
    let verify_token = base64::prelude::BASE64_URL_SAFE.decode(register_data.token)?;
    let query = VerifyAccount {
        verify_token: &verify_token,
    };
    let mut connection = pool.acquire().await?;
    let query_res = query
        .query_mysql(&shared.mysql.complete_register_statement)
        .execute(connection.acquire().await?)
        .await?;
    anyhow::ensure!(
        query_res.rows_affected() >= 1,
        "Account was not verified by the last query."
    );

    Ok(())
}
