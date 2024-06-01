use account_sql::query::Query;
use accounts_base::account_server::account_id::AccountId;
use anyhow::anyhow;
use sqlx::mysql::MySqlRow;
use sqlx::Executor;
use sqlx::Row;
use sqlx::Statement;

#[derive(Debug)]
pub struct GetAccountIdResetCode<'a> {
    pub reset_code: &'a [u8],
}

#[derive(Debug)]
pub struct GetAccountIdResetCodeData {
    pub account_id: AccountId,
}

#[async_trait::async_trait]
impl<'a> Query<GetAccountIdResetCodeData> for GetAccountIdResetCode<'a> {
    async fn prepare_mysql(
        connection: &mut sqlx::MySqlConnection,
    ) -> anyhow::Result<sqlx::mysql::MySqlStatement<'static>> {
        Ok(connection
            .prepare(include_str!("get_account_id.sql"))
            .await?)
    }
    fn query_mysql<'b>(
        &'b self,
        statement: &'b sqlx::mysql::MySqlStatement<'static>,
    ) -> sqlx::query::Query<'b, sqlx::MySql, sqlx::mysql::MySqlArguments> {
        statement.query().bind(self.reset_code)
    }
    fn row_data(row: &MySqlRow) -> anyhow::Result<GetAccountIdResetCodeData> {
        Ok(GetAccountIdResetCodeData {
            account_id: row.try_get("id")?,
        })
    }
}

#[derive(Debug)]
pub struct RemClientKeys<'a> {
    pub account_id: &'a AccountId,
}

#[async_trait::async_trait]
impl<'a> Query<()> for RemClientKeys<'a> {
    async fn prepare_mysql(
        connection: &mut sqlx::MySqlConnection,
    ) -> anyhow::Result<sqlx::mysql::MySqlStatement<'static>> {
        Ok(connection.prepare(include_str!("rem_keys.sql")).await?)
    }
    fn query_mysql<'b>(
        &'b self,
        statement: &'b sqlx::mysql::MySqlStatement<'static>,
    ) -> sqlx::query::Query<'b, sqlx::MySql, sqlx::mysql::MySqlArguments> {
        statement.query().bind(self.account_id)
    }
    fn row_data(_row: &MySqlRow) -> anyhow::Result<()> {
        Err(anyhow!("Row data is not supported"))
    }
}

#[derive(Debug)]
pub struct RemGameServerKeys<'a> {
    pub account_id: &'a AccountId,
}

#[async_trait::async_trait]
impl<'a> Query<()> for RemGameServerKeys<'a> {
    async fn prepare_mysql(
        connection: &mut sqlx::MySqlConnection,
    ) -> anyhow::Result<sqlx::mysql::MySqlStatement<'static>> {
        Ok(connection
            .prepare(include_str!("rem_game_server_key.sql"))
            .await?)
    }
    fn query_mysql<'b>(
        &'b self,
        statement: &'b sqlx::mysql::MySqlStatement<'static>,
    ) -> sqlx::query::Query<'b, sqlx::MySql, sqlx::mysql::MySqlArguments> {
        statement.query().bind(self.account_id)
    }
    fn row_data(_row: &MySqlRow) -> anyhow::Result<()> {
        Err(anyhow!("Row data is not supported"))
    }
}

#[derive(Debug)]
pub struct RemSessions<'a> {
    pub account_id: &'a AccountId,
}

#[async_trait::async_trait]
impl<'a> Query<()> for RemSessions<'a> {
    async fn prepare_mysql(
        connection: &mut sqlx::MySqlConnection,
    ) -> anyhow::Result<sqlx::mysql::MySqlStatement<'static>> {
        Ok(connection.prepare(include_str!("rem_sessions.sql")).await?)
    }
    fn query_mysql<'b>(
        &'b self,
        statement: &'b sqlx::mysql::MySqlStatement<'static>,
    ) -> sqlx::query::Query<'b, sqlx::MySql, sqlx::mysql::MySqlArguments> {
        statement.query().bind(self.account_id)
    }
    fn row_data(_row: &MySqlRow) -> anyhow::Result<()> {
        Err(anyhow!("Row data is not supported"))
    }
}
