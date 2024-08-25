use account_sql::query::Query;
use accounts_types::account_id::AccountId;
use anyhow::anyhow;
use async_trait::async_trait;
use sqlx::any::AnyRow;
use sqlx::Executor;
use sqlx::Statement;

/// A query that tries to insert a new user in the database.
/// On failure it does nothing.
#[derive(Debug)]
pub struct RenameUser<'a> {
    /// the id of the user's account, see [`AccountId`]
    pub account_id: &'a AccountId,
    /// the new name in pure ascii.
    pub name: &'a str,
}

#[async_trait]
impl<'a> Query<()> for RenameUser<'a> {
    async fn prepare_mysql(
        connection: &mut sqlx::AnyConnection,
    ) -> anyhow::Result<sqlx::any::AnyStatement<'static>> {
        Ok(connection
            .prepare(include_str!("mysql/try_rename.sql"))
            .await?)
    }
    fn query_mysql<'b>(
        &'b self,
        statement: &'b sqlx::any::AnyStatement<'static>,
    ) -> sqlx::query::Query<'b, sqlx::Any, sqlx::any::AnyArguments> {
        let account_id = self.account_id;

        statement.query().bind(self.name).bind(account_id)
    }
    fn row_data(_row: &AnyRow) -> anyhow::Result<()> {
        Err(anyhow!(
            "Data rows are not supported for this query.
            You probably want to check affected rows instead."
        ))
    }
}
