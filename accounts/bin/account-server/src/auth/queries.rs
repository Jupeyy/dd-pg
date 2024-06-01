use account_sql::query::Query;
use accounts_base::account_server::account_id::AccountId;
use accounts_base::client::auth::AuthRequest;
use sqlx::mysql::MySqlRow;
use sqlx::Executor;
use sqlx::Row;
use sqlx::Statement;

#[derive(Debug)]
pub struct AuthAttempt<'a> {
    pub data: &'a AuthRequest,
}

#[derive(Debug)]
pub struct AuthAttemptData {
    pub account_id: AccountId,
    pub serialized_secret: Vec<u8>,
    pub verified: bool,
}

#[async_trait::async_trait]
impl<'a> Query<AuthAttemptData> for AuthAttempt<'a> {
    async fn prepare_mysql(
        connection: &mut sqlx::MySqlConnection,
    ) -> anyhow::Result<sqlx::mysql::MySqlStatement<'static>> {
        Ok(connection.prepare(include_str!("auth.sql")).await?)
    }
    fn query_mysql<'b>(
        &'b self,
        statement: &'b sqlx::mysql::MySqlStatement<'static>,
    ) -> sqlx::query::Query<'b, sqlx::MySql, sqlx::mysql::MySqlArguments> {
        statement
            .query()
            .bind(self.data.pub_key.as_bytes().as_slice())
            .bind(self.data.hw_id.as_slice())
    }
    fn row_data(row: &MySqlRow) -> anyhow::Result<AuthAttemptData> {
        Ok(AuthAttemptData {
            account_id: row.try_get("account_id")?,
            serialized_secret: row.try_get("secret")?,
            verified: row.try_get("verified")?,
        })
    }
}
