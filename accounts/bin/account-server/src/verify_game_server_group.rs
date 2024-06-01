pub mod queries;

use std::sync::Arc;

use account_sql::query::Query;
use accounts_base::account_server::account_id::AccountId;
use axum::{extract, response};
use serde::Deserialize;
use sqlx::{Acquire, MySqlPool};

use crate::{
    internal_err::InternalErr, shared::Shared,
    verify_game_server_group::queries::VerifyAccountGameServerGroup,
};

#[derive(Debug, Deserialize)]
pub struct AdminAccountVerifyGameServerGroup {
    pub(crate) admin_password: String,
    pub(crate) account_id: AccountId,
}

pub async fn admin_account_verify_game_server_group(
    shared: Arc<Shared>,
    pool: MySqlPool,
    extract::Query(data): extract::Query<AdminAccountVerifyGameServerGroup>,
) -> response::Result<()> {
    admin_account_verify_game_server_group_impl(shared, pool, data)
        .await
        .map_err(|err| InternalErr(("complete_register".into(), err)).into())
}

pub async fn admin_account_verify_game_server_group_impl(
    shared: Arc<Shared>,
    pool: MySqlPool,
    data: AdminAccountVerifyGameServerGroup,
) -> anyhow::Result<()> {
    anyhow::ensure!(
        data.admin_password == shared.admin_password,
        "admin password wrong."
    );

    // try to verify the account as game server group
    let query = VerifyAccountGameServerGroup {
        account_id: &data.account_id,
    };
    let mut connection = pool.acquire().await?;
    let query_res = query
        .query_mysql(
            &shared
                .mysql
                .admin_verify_account_game_server_group_statement,
        )
        .execute(connection.acquire().await?)
        .await?;
    anyhow::ensure!(
        query_res.rows_affected() >= 1,
        "Account was not verified as game server group by the last query."
    );

    Ok(())
}
