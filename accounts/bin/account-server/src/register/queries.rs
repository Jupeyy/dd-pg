use account_sql::query::Query;
use accounts_base::client::register::RegisterDataForServer;
use anyhow::anyhow;
use argon2::password_hash::SaltString;
use sqlx::mysql::MySqlRow;
use sqlx::Executor;
use sqlx::Statement;

#[derive(Debug)]
pub struct AddAccount<'a> {
    pub data: &'a RegisterDataForServer,
    pub password_hash: [u8; 32],
    pub salt: SaltString,
    pub serialized_main_secret: Vec<u8>,
}

#[async_trait::async_trait]
impl<'a> Query<()> for AddAccount<'a> {
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
            .bind(self.data.email.as_str())
            .bind(self.password_hash.as_slice())
            .bind(self.salt.as_str())
            .bind(self.serialized_main_secret.as_slice())
    }
    fn row_data(_row: &MySqlRow) -> anyhow::Result<()> {
        Err(anyhow!("Row data is not supported"))
    }
}

#[derive(Debug)]
pub struct AddVerifyToken<'a> {
    pub verify_token: &'a [u8; 32],
}

#[async_trait::async_trait]
impl<'a> Query<()> for AddVerifyToken<'a> {
    async fn prepare_mysql(
        connection: &mut sqlx::MySqlConnection,
    ) -> anyhow::Result<sqlx::mysql::MySqlStatement<'static>> {
        Ok(connection
            .prepare(include_str!("add_verify_token.sql"))
            .await?)
    }
    fn query_mysql<'b>(
        &'b self,
        statement: &'b sqlx::mysql::MySqlStatement<'static>,
    ) -> sqlx::query::Query<'b, sqlx::MySql, sqlx::mysql::MySqlArguments> {
        statement.query().bind(self.verify_token.as_slice())
    }
    fn row_data(_row: &MySqlRow) -> anyhow::Result<()> {
        Err(anyhow!("Row data is not supported"))
    }
}
