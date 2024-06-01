use std::sync::Arc;

use account_sql::query::Query;
use sqlx::Acquire;

use crate::{
    auto_login::queries::{AutoLogin, RegisterUser},
    mysql::MySqlConnectionShared,
    register::queries::Register,
    shared::Shared,
};

async fn prepare_mysql(pool: &sqlx::MySqlPool) -> anyhow::Result<MySqlConnectionShared> {
    let mut pool_con = pool.acquire().await?;
    let con = pool_con.acquire().await?;

    Ok(MySqlConnectionShared {
        register_statement: Register::prepare_mysql(con).await?,
        auto_login_statement: AutoLogin::prepare_mysql(con).await?,
        register_user_statement: RegisterUser::prepare_mysql(con).await?,
    })
}

/// Prepare shared data used in the game server's implementation
pub async fn prepare(pool: &sqlx::MySqlPool) -> anyhow::Result<Arc<Shared>> {
    Ok(Arc::new(Shared {
        mysql: prepare_mysql(pool).await?,
    }))
}
