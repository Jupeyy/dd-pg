use account_sql::query::Query;
use accounts_base::account_server::account_id::AccountId;
use accounts_base::game_server::user_id::UserId;
use anyhow::anyhow;
use async_trait::async_trait;
use sqlx::mysql::MySqlRow;
use sqlx::Executor;
use sqlx::Row;
use sqlx::Statement;

/// A query that checks if a given user_id
/// exists and returns the account id (if any).
#[derive(Debug)]
pub struct AutoLogin<'a> {
    /// the id of the user, see [`UserId`]
    pub user_id: &'a UserId,
}

pub struct AutoLoginData {
    /// The account id that was found for the user id
    pub account_id: Option<AccountId>,
}

#[async_trait]
impl<'a> Query<AutoLoginData> for AutoLogin<'a> {
    async fn prepare_mysql(
        connection: &mut sqlx::MySqlConnection,
    ) -> anyhow::Result<sqlx::mysql::MySqlStatement<'static>> {
        Ok(connection.prepare(include_str!("auto_login.sql")).await?)
    }
    fn query_mysql<'b>(
        &'b self,
        statement: &'b sqlx::mysql::MySqlStatement<'static>,
    ) -> sqlx::query::Query<'b, sqlx::MySql, sqlx::mysql::MySqlArguments> {
        statement.query().bind(self.user_id.as_slice())
    }
    fn row_data(row: &MySqlRow) -> anyhow::Result<AutoLoginData> {
        Ok(AutoLoginData {
            account_id: row.try_get("account_id")?,
        })
    }
}

/// A query that tries to insert a new user in the database.
/// On failure it does nothing.
#[derive(Debug)]
pub struct RegisterUser<'a> {
    /// the id of the user, see [`UserId`]
    pub user_id: &'a UserId,
}

#[async_trait]
impl<'a> Query<()> for RegisterUser<'a> {
    async fn prepare_mysql(
        connection: &mut sqlx::MySqlConnection,
    ) -> anyhow::Result<sqlx::mysql::MySqlStatement<'static>> {
        Ok(connection
            .prepare(include_str!("try_insert_user.sql"))
            .await?)
    }
    fn query_mysql<'b>(
        &'b self,
        statement: &'b sqlx::mysql::MySqlStatement<'static>,
    ) -> sqlx::query::Query<'b, sqlx::MySql, sqlx::mysql::MySqlArguments> {
        statement.query().bind(self.user_id.as_slice())
    }
    fn row_data(_row: &MySqlRow) -> anyhow::Result<()> {
        Err(anyhow!(
            "Data rows are not supported for this query.
            You probably want to check affected rows instead."
        ))
    }
}
