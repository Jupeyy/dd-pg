pub(crate) mod queries;

use std::sync::Arc;

use account_sql::query::Query;
use accounts_shared::game_server::user_id::UserId;
use sqlx::Acquire;
use thiserror::Error;

use crate::{
    auto_login::{default_name, DEFAULT_NAME_PREFIX},
    shared::Shared,
};

use self::queries::RenameUser;

/// The error type if registering to the game server fails.
#[derive(Error, Debug)]
pub enum RenameError {
    /// A database error happened.
    #[error("{0}")]
    Database(anyhow::Error),
    /// only specific ascii characters are allowed.
    #[error("only ascii characters [a-z], [A-Z] and [0-9] are allowed.")]
    InvalidAscii,
    /// some names are not allowed.
    #[error("a user name is not allowed to start with \"autouser\".")]
    ReservedName,
    /// the user name is already taken
    #[error("a user with that name already exists.")]
    UserNameAlreadyExists,
}

/// Renames a user.
/// Returns `true` if the rename was successful.
/// Returns `false` if the user had no account.
pub async fn rename(
    shared: Arc<Shared>,
    pool: &sqlx::AnyPool,
    user_id: &UserId,
    name: &str,
) -> anyhow::Result<bool, RenameError> {
    if let Some(account_id) = &user_id.account_id {
        name.chars()
            .all(|char| char.is_ascii_alphanumeric())
            .then_some(())
            .ok_or_else(|| RenameError::InvalidAscii)?;
        // renaming back to the default name is allowed
        (!name.starts_with(DEFAULT_NAME_PREFIX) || name == default_name(account_id))
            .then_some(())
            .ok_or_else(|| RenameError::ReservedName)?;

        let mut pool_con = pool
            .acquire()
            .await
            .map_err(|err| RenameError::Database(err.into()))?;
        let con = pool_con
            .acquire()
            .await
            .map_err(|err| RenameError::Database(err.into()))?;

        let qry = RenameUser { account_id, name };

        let res = qry
            .query(&shared.db.register_user_statement)
            .execute(&mut *con)
            .await
            .map_err(|err| RenameError::Database(err.into()))?;

        (res.rows_affected() >= 1)
            .then_some(())
            .ok_or_else(|| RenameError::UserNameAlreadyExists)?;

        Ok(true)
    } else {
        Ok(false)
    }
}
