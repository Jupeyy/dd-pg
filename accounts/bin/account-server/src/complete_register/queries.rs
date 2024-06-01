use account_sql::query::Query;
use anyhow::anyhow;
use sqlx::mysql::MySqlRow;
use sqlx::Executor;
use sqlx::Statement;

#[derive(Debug)]
pub struct VerifyAccount<'a> {
    pub verify_token: &'a [u8],
}

#[async_trait::async_trait]
impl<'a> Query<()> for VerifyAccount<'a> {
    async fn prepare_mysql(
        connection: &mut sqlx::MySqlConnection,
    ) -> anyhow::Result<sqlx::mysql::MySqlStatement<'static>> {
        Ok(connection.prepare(include_str!("verify.sql")).await?)
    }
    fn query_mysql<'b>(
        &'b self,
        statement: &'b sqlx::mysql::MySqlStatement<'static>,
    ) -> sqlx::query::Query<'b, sqlx::MySql, sqlx::mysql::MySqlArguments> {
        statement.query().bind(self.verify_token)
    }
    fn row_data(_row: &MySqlRow) -> anyhow::Result<()> {
        Err(anyhow!("Row data is not supported"))
    }
}
