use account_sql::query::Query;
use accounts_base::account_server::account_id::AccountId;
use accounts_base::client::session::SessionDataForServer;
use anyhow::anyhow;
use argon2::password_hash::SaltString;
use axum::async_trait;
use sqlx::mysql::MySqlRow;
use sqlx::Executor;
use sqlx::Row;
use sqlx::Statement;

pub struct LoginAttempt<'a> {
    pub data: &'a SessionDataForServer,
}

pub struct LoginAttempData {
    pub account_id: AccountId,
    pub password: [u8; 32],
    pub salt: SaltString,
    pub serialized_encrypted_main_secret: Vec<u8>,
}

#[async_trait]
impl<'a> Query<LoginAttempData> for LoginAttempt<'a> {
    async fn prepare_mysql(
        connection: &mut sqlx::MySqlConnection,
    ) -> anyhow::Result<sqlx::mysql::MySqlStatement<'static>> {
        Ok(connection.prepare(include_str!("login_data.sql")).await?)
    }
    fn query_mysql<'b>(
        &'b self,
        statement: &'b sqlx::mysql::MySqlStatement<'static>,
    ) -> sqlx::query::Query<'b, sqlx::MySql, sqlx::mysql::MySqlArguments> {
        statement.query().bind(self.data.email.as_str())
    }
    fn row_data(row: &MySqlRow) -> anyhow::Result<LoginAttempData> {
        let password: Vec<u8> = row.try_get("password")?;
        Ok(LoginAttempData {
            account_id: row
                .try_get("id")
                .map_err(|err| anyhow!("Failed get column id: {err}"))?,
            password: password.try_into().map_err(|_| {
                anyhow!("failed to convert Vec<u8> to [u8; 32] while a login attempt.")
            })?,
            salt: SaltString::from_b64(row.try_get("salt")?).map_err(|err| anyhow!(err))?,
            serialized_encrypted_main_secret: row.try_get("encrypted_main_secret")?,
        })
    }
}

pub struct CreateSession<'a> {
    pub account_id: AccountId,
    pub pub_key: &'a [u8; ed25519_dalek::PUBLIC_KEY_LENGTH],
    pub serialized_secret: Vec<u8>,
    pub hw_id: &'a [u8; 32],
}

#[async_trait]
impl<'a> Query<()> for CreateSession<'a> {
    async fn prepare_mysql(
        connection: &mut sqlx::MySqlConnection,
    ) -> anyhow::Result<sqlx::mysql::MySqlStatement<'static>> {
        Ok(connection.prepare(include_str!("add_session.sql")).await?)
    }
    fn query_mysql<'b>(
        &'b self,
        statement: &'b sqlx::mysql::MySqlStatement<'static>,
    ) -> sqlx::query::Query<'b, sqlx::MySql, sqlx::mysql::MySqlArguments> {
        statement
            .query()
            .bind(self.account_id)
            .bind(self.pub_key.as_slice())
            .bind(self.serialized_secret.as_slice())
            .bind(self.hw_id.as_slice())
    }
    fn row_data(_row: &MySqlRow) -> anyhow::Result<()> {
        Err(anyhow!("Row data is not supported"))
    }
}
