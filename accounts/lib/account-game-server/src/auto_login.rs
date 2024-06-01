pub(crate) mod queries;

use std::sync::Arc;

use account_sql::query::Query;
use accounts_base::{account_server::account_id::AccountId, game_server::user_id::UserId};
use serde::{Deserialize, Serialize};
use sqlx::Acquire;
use thiserror::Error;

use crate::shared::Shared;

use self::queries::{AutoLogin, RegisterUser};

/// The error type if registering to the game server fails.
#[derive(Error, Debug)]
pub enum AutoLoginError {
    /// The user has to register now.
    /// If not no ranks will be saved.
    /// The reason for this is that the user can be
    /// upgraded to an user with account id only once.
    #[error("User must register now.")]
    MustRegister,
    /// A database error happened.
    #[error("{0}")]
    Database(anyhow::Error),
}

/// The result of the auto login.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoLoginResult {
    /// The optional account id, if the user
    /// was already registered on this game server.    
    pub account_id: Option<AccountId>,
}

/// Logges in the user.
/// Might create a new user row if the user didn't exist before.
pub async fn auto_login(
    shared: Arc<Shared>,
    pool: &sqlx::MySqlPool,
    user_id: &UserId,
    has_account: bool,
) -> anyhow::Result<AutoLoginResult, AutoLoginError> {
    let mut pool_con = pool
        .acquire()
        .await
        .map_err(|err| AutoLoginError::Database(err.into()))?;
    let con = pool_con
        .acquire()
        .await
        .map_err(|err| AutoLoginError::Database(err.into()))?;

    let qry = AutoLogin { user_id };
    let qry_res = qry
        .query_mysql(&shared.mysql.auto_login_statement)
        .fetch_one(&mut *con)
        .await;

    match qry_res {
        Ok(row) => {
            let data = AutoLogin::row_data(&row).map_err(AutoLoginError::Database)?;
            Ok(AutoLoginResult {
                account_id: data.account_id,
            })
        }
        Err(err) => match err {
            sqlx::Error::RowNotFound => {
                if has_account {
                    Err(AutoLoginError::MustRegister)
                } else {
                    // try to add user instead
                    let qry = RegisterUser { user_id };
                    let qry_res = qry
                        .query_mysql(&shared.mysql.register_user_statement)
                        .execute(&mut *con)
                        .await;

                    match qry_res {
                        Ok(_) => {
                            // Nothing to do.
                        }
                        Err(err) => {
                            match err {
                                sqlx::Error::Database(err) => match err.kind() {
                                    sqlx::error::ErrorKind::UniqueViolation => {
                                        // user seems to be already registed, simply ignore
                                        // the error.
                                    }
                                    _ => {
                                        return Err(err)
                                            .map_err(|err| AutoLoginError::Database(err.into()))
                                    }
                                },
                                _ => {
                                    return Err(err)
                                        .map_err(|err| AutoLoginError::Database(err.into()))
                                }
                            }
                        }
                    }

                    Ok(AutoLoginResult { account_id: None })
                }
            }
            _ => Err(err).map_err(|err| AutoLoginError::Database(err.into())),
        },
    }
}
