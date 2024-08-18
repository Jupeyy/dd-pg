use account_sql::query::Query;
use accounts_types::account_id::AccountId;
use anyhow::anyhow;
use async_trait::async_trait;
use sqlx::any::AnyRow;
use sqlx::Executor;
use sqlx::Row;
use sqlx::Statement;

#[derive(Debug)]
pub struct AddAccountTokenEmail<'a> {
    pub token: &'a [u8; 32],
    pub email: &'a email_address::EmailAddress,
}

#[async_trait]
impl<'a> Query<()> for AddAccountTokenEmail<'a> {
    async fn prepare_mysql(
        connection: &mut sqlx::AnyConnection,
    ) -> anyhow::Result<sqlx::any::AnyStatement<'static>> {
        Ok(connection
            .prepare(include_str!("mysql/add_account_token_email.sql"))
            .await?)
    }
    fn query_mysql<'b>(
        &'b self,
        statement: &'b sqlx::any::AnyStatement<'static>,
    ) -> sqlx::query::Query<'b, sqlx::Any, sqlx::any::AnyArguments> {
        statement
            .query()
            .bind(self.token.as_slice())
            .bind(self.email.as_str())
    }
    fn row_data(_row: &AnyRow) -> anyhow::Result<()> {
        Err(anyhow!("Row data is not supported"))
    }
}

pub struct AccountTokenQry<'a> {
    pub token: &'a [u8; 32],
}

pub struct AccountTokenData {
    pub account_id: AccountId,
}

#[async_trait]
impl<'a> Query<AccountTokenData> for AccountTokenQry<'a> {
    async fn prepare_mysql(
        connection: &mut sqlx::AnyConnection,
    ) -> anyhow::Result<sqlx::any::AnyStatement<'static>> {
        Ok(connection
            .prepare(include_str!("mysql/account_token_data.sql"))
            .await?)
    }
    fn query_mysql<'b>(
        &'b self,
        statement: &'b sqlx::any::AnyStatement<'static>,
    ) -> sqlx::query::Query<'b, sqlx::Any, sqlx::any::AnyArguments> {
        statement.query().bind(self.token.as_slice())
    }
    fn row_data(row: &AnyRow) -> anyhow::Result<AccountTokenData> {
        Ok(AccountTokenData {
            account_id: row
                .try_get("accound_id")
                .map_err(|err| anyhow!("Failed get column account_id: {err}"))?,
        })
    }
}

pub struct InvalidateAccountToken<'a> {
    pub token: &'a [u8; 32],
}

#[async_trait]
impl<'a> Query<()> for InvalidateAccountToken<'a> {
    async fn prepare_mysql(
        connection: &mut sqlx::AnyConnection,
    ) -> anyhow::Result<sqlx::any::AnyStatement<'static>> {
        Ok(connection
            .prepare(include_str!("mysql/invalidate_account_token.sql"))
            .await?)
    }
    fn query_mysql<'b>(
        &'b self,
        statement: &'b sqlx::any::AnyStatement<'static>,
    ) -> sqlx::query::Query<'b, sqlx::Any, sqlx::any::AnyArguments> {
        statement.query().bind(self.token.as_slice())
    }
    fn row_data(_row: &AnyRow) -> anyhow::Result<()> {
        Err(anyhow!("Row data is not supported"))
    }
}
