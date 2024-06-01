use account_sql::query::Query;
use accounts_base::account_server::account_id::AccountId;
use accounts_base::game_server::user_id::UserId;
use anyhow::anyhow;
use async_trait::async_trait;
use sqlx::mysql::MySqlRow;
use sqlx::Executor;
use sqlx::Statement;

/// A query that registeres a new user
/// in the data base. If the user already
/// exists, then the account id is updated.
#[derive(Debug)]
pub struct Register<'a> {
    /// the id of the user, see [`UserId`]
    pub user_id: &'a UserId,
    /// the account id of the user
    pub account_id: AccountId,
}

#[async_trait]
impl<'a> Query<()> for Register<'a> {
    async fn prepare_mysql(
        connection: &mut sqlx::MySqlConnection,
    ) -> anyhow::Result<sqlx::mysql::MySqlStatement<'static>> {
        Ok(connection.prepare(include_str!("register.sql")).await?)
    }
    fn query_mysql<'b>(
        &'b self,
        statement: &'b sqlx::mysql::MySqlStatement<'static>,
    ) -> sqlx::query::Query<'b, sqlx::MySql, sqlx::mysql::MySqlArguments> {
        statement
            .query()
            .bind(self.user_id.as_slice())
            .bind(self.account_id)
            .bind(self.user_id.as_slice())
            .bind(self.account_id)
    }
    fn row_data(_row: &MySqlRow) -> anyhow::Result<()> {
        Err(anyhow!(
            "Data rows are not supported for this query.
            You probably want to check affected rows instead."
        ))
    }
}
