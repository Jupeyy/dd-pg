pub(crate) mod queries;

use std::sync::Arc;

use account_sql::query::Query;
use accounts_base::game_server::user_id::UserId;
use sqlx::Acquire;
use thiserror::Error;

use crate::shared::Shared;

use self::queries::RegisterUser;

/// The error type if registering to the game server fails.
#[derive(Error, Debug)]
pub enum AutoLoginError {
    /// A database error happened.
    #[error("{0}")]
    Database(anyhow::Error),
}

/// Logges in the user.
/// Might create a new user row if the user didn't exist before.
pub async fn auto_login(
    shared: Arc<Shared>,
    pool: &sqlx::MySqlPool,
    user_id: &UserId,
) -> anyhow::Result<(), AutoLoginError> {
    let mut pool_con = pool
        .acquire()
        .await
        .map_err(|err| AutoLoginError::Database(err.into()))?;
    let con = pool_con
        .acquire()
        .await
        .map_err(|err| AutoLoginError::Database(err.into()))?;

    let qry = RegisterUser { user_id };

    qry.query_mysql(&shared.mysql.register_user_statement)
        .execute(&mut *con)
        .await
        .map_err(|err| AutoLoginError::Database(err.into()))?;

    Ok(())
}
