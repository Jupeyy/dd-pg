use account_sql::query::Query;
use accounts_base::game_server::user_id::UserId;
use anyhow::anyhow;
use async_trait::async_trait;
use sqlx::mysql::MySqlRow;
use sqlx::Executor;
use sqlx::Statement;

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
        let public_key = self
            .user_id
            .account_id
            .is_none()
            .then_some(self.user_id.public_key.as_slice());
        let account_id = self.user_id.account_id;

        statement
            .query()
            .bind(public_key)
            .bind(account_id)
            .bind(account_id)
            .bind(public_key)
            .bind(account_id)
    }
    fn row_data(_row: &MySqlRow) -> anyhow::Result<()> {
        Err(anyhow!(
            "Data rows are not supported for this query.
            You probably want to check affected rows instead."
        ))
    }
}
