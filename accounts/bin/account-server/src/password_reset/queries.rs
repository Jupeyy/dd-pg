use account_sql::query::Query;
use anyhow::anyhow;
use argon2::password_hash::SaltString;
use sqlx::mysql::MySqlRow;
use sqlx::Executor;
use sqlx::Statement;

#[derive(Debug)]
pub struct VerifyResetCodeAndResetAccount<'a> {
    pub reset_code: &'a [u8],
    pub password_hash: [u8; 32],
    pub salt: SaltString,
    pub serialized_main_secret: Vec<u8>,
}

#[async_trait::async_trait]
impl<'a> Query<()> for VerifyResetCodeAndResetAccount<'a> {
    async fn prepare_mysql(
        connection: &mut sqlx::MySqlConnection,
    ) -> anyhow::Result<sqlx::mysql::MySqlStatement<'static>> {
        Ok(connection
            .prepare(include_str!("reset_account.sql"))
            .await?)
    }
    fn query_mysql<'b>(
        &'b self,
        statement: &'b sqlx::mysql::MySqlStatement<'static>,
    ) -> sqlx::query::Query<'b, sqlx::MySql, sqlx::mysql::MySqlArguments> {
        statement
            .query()
            .bind(self.password_hash.as_slice())
            .bind(self.salt.as_str())
            .bind(self.serialized_main_secret.as_slice())
            .bind(self.reset_code)
    }
    fn row_data(_row: &MySqlRow) -> anyhow::Result<()> {
        Err(anyhow!("Row data is not supported"))
    }
}
