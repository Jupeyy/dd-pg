pub mod queries;

use std::sync::Arc;

use account_sql::query::Query;
use accounts_shared::client::delete::DeleteRequest;
use axum::{response, Json};
use sqlx::{Acquire, AnyPool, Connection};

use crate::{
    account_token::queries::{AccountTokenQry, InvalidateAccountToken},
    internal_err::InternalErr,
    shared::Shared,
};

use self::queries::{RemoveAccount, RemoveSessions};

pub async fn delete_request(
    shared: Arc<Shared>,
    pool: AnyPool,
    Json(data): Json<DeleteRequest>,
) -> response::Result<Json<()>> {
    delete(shared, pool, data)
        .await
        .map_err(|err| InternalErr(("delete".into(), err)).into())
        .map(Json)
}

pub async fn delete(shared: Arc<Shared>, pool: AnyPool, data: DeleteRequest) -> anyhow::Result<()> {
    let mut connection = pool.acquire().await?;
    let connection = connection.acquire().await?;

    connection
        .transaction(|connection| {
            Box::pin(async move {
                // token data
                let acc_token_qry = AccountTokenQry {
                    token: &data.account_token,
                };

                let row = acc_token_qry
                    .query(&shared.db.account_token_qry_statement)
                    .fetch_one(&mut **connection)
                    .await?;

                let token_data = AccountTokenQry::row_data(&row)?;

                // invalidate token
                let qry = InvalidateAccountToken {
                    token: &data.account_token,
                };
                qry.query(&shared.db.invalidate_account_token_statement)
                    .execute(&mut **connection)
                    .await?;

                let account_id = token_data.account_id;

                // remove all sessions
                let qry = RemoveSessions {
                    account_id: &account_id,
                };

                qry.query(&shared.db.remove_sessions_statement)
                    .execute(&mut **connection)
                    .await?;

                // delete account
                let qry = RemoveAccount {
                    account_id: &account_id,
                };

                qry.query(&shared.db.remove_account_statement)
                    .execute(&mut **connection)
                    .await?;

                anyhow::Ok(())
            })
        })
        .await?;

    Ok(())
}

pub async fn delete_sessions_request(
    shared: Arc<Shared>,
    pool: AnyPool,
    Json(data): Json<DeleteRequest>,
) -> response::Result<Json<()>> {
    delete_sessions(shared, pool, data)
        .await
        .map_err(|err| InternalErr(("delete_sessions".into(), err)).into())
        .map(Json)
}

pub async fn delete_sessions(
    shared: Arc<Shared>,
    pool: AnyPool,
    data: DeleteRequest,
) -> anyhow::Result<()> {
    let mut connection = pool.acquire().await?;
    let connection = connection.acquire().await?;

    connection
        .transaction(|connection| {
            Box::pin(async move {
                // token data
                let acc_token_qry = AccountTokenQry {
                    token: &data.account_token,
                };

                let row = acc_token_qry
                    .query(&shared.db.account_token_qry_statement)
                    .fetch_one(&mut **connection)
                    .await?;

                let token_data = AccountTokenQry::row_data(&row)?;

                // invalidate token
                let qry = InvalidateAccountToken {
                    token: &data.account_token,
                };
                qry.query(&shared.db.invalidate_account_token_statement)
                    .execute(&mut **connection)
                    .await?;

                let account_id = token_data.account_id;

                // remove all sessions
                let qry = RemoveSessions {
                    account_id: &account_id,
                };

                qry.query(&shared.db.remove_sessions_statement)
                    .execute(&mut **connection)
                    .await?;

                // delete account
                let qry = RemoveAccount {
                    account_id: &account_id,
                };

                qry.query(&shared.db.remove_account_statement)
                    .execute(&mut **connection)
                    .await?;

                anyhow::Ok(())
            })
        })
        .await?;

    Ok(())
}
