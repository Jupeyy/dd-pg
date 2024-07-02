use account_sql::query::Query;
use anyhow::anyhow;
use sqlx::mysql::MySqlRow;
use sqlx::Executor;
use sqlx::Statement;

#[derive(Debug)]
pub struct AddLoginToken<'a> {
    pub token: &'a [u8; 32],
    pub email: &'a email_address::EmailAddress,
}

#[async_trait::async_trait]
impl<'a> Query<()> for AddLoginToken<'a> {
    async fn prepare_mysql(
        connection: &mut sqlx::MySqlConnection,
    ) -> anyhow::Result<sqlx::mysql::MySqlStatement<'static>> {
        Ok(connection
            .prepare(include_str!("add_login_token.sql"))
            .await?)
    }
    fn query_mysql<'b>(
        &'b self,
        statement: &'b sqlx::mysql::MySqlStatement<'static>,
    ) -> sqlx::query::Query<'b, sqlx::MySql, sqlx::mysql::MySqlArguments> {
        statement
            .query()
            .bind(self.token.as_slice())
            .bind(self.email.as_str())
    }
    fn row_data(_row: &MySqlRow) -> anyhow::Result<()> {
        Err(anyhow!("Row data is not supported"))
    }
}
