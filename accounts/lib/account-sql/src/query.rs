use async_trait::async_trait;
use sqlx::mysql::MySqlRow;

/// An interface for queries to allow converting them to various database implementations
#[async_trait]
pub trait Query<A> {
    /// Prepare a statement to be later used by [`Query::query_mysql`].
    async fn prepare_mysql(
        connection: &mut sqlx::MySqlConnection,
    ) -> anyhow::Result<sqlx::mysql::MySqlStatement<'static>>;
    /// Get a query with all arguments bound already, ready to be fetched.
    fn query_mysql<'a>(
        &'a self,
        statement: &'a sqlx::mysql::MySqlStatement<'static>,
    ) -> sqlx::query::Query<'a, sqlx::MySql, sqlx::mysql::MySqlArguments>;
    /// Gets the row data for a result row of this query
    fn row_data(row: &MySqlRow) -> anyhow::Result<A>;
}
