use account_sql::query::Query;
use accounts_types::account_id::AccountId;
use anyhow::anyhow;
use axum::async_trait;
use sqlx::any::AnyRow;
use sqlx::Executor;
use sqlx::Row;
use sqlx::Statement;

pub struct LoginTokenQry<'a> {
    pub token: &'a [u8; 32],
}

pub struct LoginTokenData {
    pub email: Option<String>,
    pub steam_id: Option<String>,
}

#[async_trait]
impl<'a> Query<LoginTokenData> for LoginTokenQry<'a> {
    async fn prepare_mysql(
        connection: &mut sqlx::AnyConnection,
    ) -> anyhow::Result<sqlx::any::AnyStatement<'static>> {
        Ok(connection
            .prepare(include_str!("mysql/login_token_data.sql"))
            .await?)
    }
    fn query_mysql<'b>(
        &'b self,
        statement: &'b sqlx::any::AnyStatement<'static>,
    ) -> sqlx::query::Query<'b, sqlx::Any, sqlx::any::AnyArguments> {
        statement.query().bind(self.token.as_slice())
    }
    fn row_data(row: &AnyRow) -> anyhow::Result<LoginTokenData> {
        Ok(LoginTokenData {
            email: row
                .try_get("email")
                .map_err(|err| anyhow!("Failed get column email: {err}"))?,
            steam_id: row
                .try_get("steamid")
                .map_err(|err| anyhow!("Failed get column steamid: {err}"))?,
        })
    }
}

pub struct InvalidateLoginToken<'a> {
    pub token: &'a [u8; 32],
}

#[async_trait]
impl<'a> Query<()> for InvalidateLoginToken<'a> {
    async fn prepare_mysql(
        connection: &mut sqlx::AnyConnection,
    ) -> anyhow::Result<sqlx::any::AnyStatement<'static>> {
        Ok(connection
            .prepare(include_str!("mysql/invalidate_login_token.sql"))
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

pub struct TryCreateAccount<'a> {
    pub email: &'a Option<email_address::EmailAddress>,
    pub steam_id: &'a Option<String>,
}

#[async_trait]
impl<'a> Query<()> for TryCreateAccount<'a> {
    async fn prepare_mysql(
        connection: &mut sqlx::AnyConnection,
    ) -> anyhow::Result<sqlx::any::AnyStatement<'static>> {
        Ok(connection
            .prepare(include_str!("mysql/add_account.sql"))
            .await?)
    }
    fn query_mysql<'b>(
        &'b self,
        statement: &'b sqlx::any::AnyStatement<'static>,
    ) -> sqlx::query::Query<'b, sqlx::Any, sqlx::any::AnyArguments> {
        statement
            .query()
            .bind(self.email.as_ref().map(|email| email.as_str()))
            .bind(self.steam_id.as_ref().map(|steamid| steamid.as_str()))
    }
    fn row_data(_row: &AnyRow) -> anyhow::Result<()> {
        Err(anyhow!("Row data is not supported"))
    }
}

pub struct LoginQry<'a> {
    pub email: &'a Option<email_address::EmailAddress>,
    pub steam_id: &'a Option<email_address::EmailAddress>,
}

pub struct LoginData {
    pub account_id: AccountId,
}

#[async_trait]
impl<'a> Query<LoginData> for LoginQry<'a> {
    async fn prepare_mysql(
        connection: &mut sqlx::AnyConnection,
    ) -> anyhow::Result<sqlx::any::AnyStatement<'static>> {
        Ok(connection
            .prepare(include_str!("mysql/login_data.sql"))
            .await?)
    }
    fn query_mysql<'b>(
        &'b self,
        statement: &'b sqlx::any::AnyStatement<'static>,
    ) -> sqlx::query::Query<'b, sqlx::Any, sqlx::any::AnyArguments> {
        statement
            .query()
            .bind(self.email.as_ref().map(|email| email.as_str()))
            .bind(self.steam_id.as_ref().map(|steamid| steamid.as_str()))
    }
    fn row_data(row: &AnyRow) -> anyhow::Result<LoginData> {
        Ok(LoginData {
            account_id: row
                .try_get("id")
                .map_err(|err| anyhow!("Failed get column account id: {err}"))?,
        })
    }
}

pub struct CreateSession<'a> {
    pub account_id: AccountId,
    pub pub_key: &'a [u8; ed25519_dalek::PUBLIC_KEY_LENGTH],
    pub hw_id: &'a [u8; 32],
}

#[async_trait]
impl<'a> Query<()> for CreateSession<'a> {
    async fn prepare_mysql(
        connection: &mut sqlx::AnyConnection,
    ) -> anyhow::Result<sqlx::any::AnyStatement<'static>> {
        Ok(connection
            .prepare(include_str!("mysql/add_session.sql"))
            .await?)
    }
    fn query_mysql<'b>(
        &'b self,
        statement: &'b sqlx::any::AnyStatement<'static>,
    ) -> sqlx::query::Query<'b, sqlx::Any, sqlx::any::AnyArguments> {
        statement
            .query()
            .bind(self.account_id)
            .bind(self.pub_key.as_slice())
            .bind(self.hw_id.as_slice())
    }
    fn row_data(_row: &AnyRow) -> anyhow::Result<()> {
        Err(anyhow!("Row data is not supported"))
    }
}
