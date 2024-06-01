use account_sql::query::Query;
use accounts_base::account_server::account_id::AccountId;
use accounts_base::account_server::reset_code::ResetCode;
use anyhow::anyhow;
use sqlx::mysql::MySqlRow;
use sqlx::Executor;
use sqlx::Row;
use sqlx::Statement;

#[derive(Debug)]
pub struct EmailExistsCheck<'a> {
    pub email: &'a email_address::EmailAddress,
}

#[derive(Debug)]
pub struct EmailAccountId {
    pub account_id: AccountId,
}

#[async_trait::async_trait]
impl<'a> Query<EmailAccountId> for EmailExistsCheck<'a> {
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
        statement.query().bind(self.email.as_str())
    }
    fn row_data(row: &MySqlRow) -> anyhow::Result<EmailAccountId> {
        Ok(EmailAccountId {
            account_id: row.try_get("id")?,
        })
    }
}

#[derive(Debug)]
pub struct AddResetCode<'a> {
    pub account_id: AccountId,
    pub reset_code: &'a ResetCode,
}

#[async_trait::async_trait]
impl<'a> Query<()> for AddResetCode<'a> {
    async fn prepare_mysql(
        connection: &mut sqlx::MySqlConnection,
    ) -> anyhow::Result<sqlx::mysql::MySqlStatement<'static>> {
        Ok(connection
            .prepare(include_str!("add_reset_code.sql"))
            .await?)
    }
    fn query_mysql<'b>(
        &'b self,
        statement: &'b sqlx::mysql::MySqlStatement<'static>,
    ) -> sqlx::query::Query<'b, sqlx::MySql, sqlx::mysql::MySqlArguments> {
        statement
            .query()
            .bind(self.account_id)
            .bind(self.reset_code.as_slice())
    }
    fn row_data(_row: &MySqlRow) -> anyhow::Result<()> {
        Err(anyhow!("Row data is not supported"))
    }
}
