use account_sql::query::Query;
use anyhow::anyhow;
use sqlx::any::AnyRow;
use sqlx::Executor;
use sqlx::Statement;

use crate::types::TokenType;

#[derive(Debug)]
pub struct AddLoginToken<'a> {
    pub token: &'a [u8; 32],
    pub ty: &'a TokenType,
    pub identifier: &'a str,
}

#[async_trait::async_trait]
impl<'a> Query<()> for AddLoginToken<'a> {
    async fn prepare_mysql(
        connection: &mut sqlx::AnyConnection,
    ) -> anyhow::Result<sqlx::any::AnyStatement<'static>> {
        Ok(connection
            .prepare(include_str!("mysql/add_login_token.sql"))
            .await?)
    }
    fn query_mysql<'b>(
        &'b self,
        statement: &'b sqlx::any::AnyStatement<'static>,
    ) -> sqlx::query::Query<'b, sqlx::Any, sqlx::any::AnyArguments> {
        let ty: &'static str = self.ty.into();
        statement
            .query()
            .bind(self.token.as_slice())
            .bind(ty)
            .bind(self.identifier)
    }
    fn row_data(_row: &AnyRow) -> anyhow::Result<()> {
        Err(anyhow!("Row data is not supported"))
    }
}
