use account_sql::query::Query;
use anyhow::anyhow;
use ed25519_dalek::VerifyingKey;
use sqlx::mysql::MySqlRow;
use sqlx::Executor;
use sqlx::Row;
use sqlx::Statement;

#[derive(Debug)]
pub struct GetGameServerGroupKeyPair<'a> {
    pub public_key: &'a VerifyingKey,
    pub hw_id: &'a [u8; 32],
}

#[derive(Debug)]
pub struct GetGameServerGroupKeyPairData {
    pub serialized_key_pair: Option<Vec<u8>>,
}

#[async_trait::async_trait]
impl<'a> Query<GetGameServerGroupKeyPairData> for GetGameServerGroupKeyPair<'a> {
    async fn prepare_mysql(
        connection: &mut sqlx::MySqlConnection,
    ) -> anyhow::Result<sqlx::mysql::MySqlStatement<'static>> {
        Ok(connection
            .prepare(include_str!("game_server_get_key.sql"))
            .await?)
    }
    fn query_mysql<'b>(
        &'b self,
        statement: &'b sqlx::mysql::MySqlStatement<'static>,
    ) -> sqlx::query::Query<'b, sqlx::MySql, sqlx::mysql::MySqlArguments> {
        statement
            .query()
            .bind(self.public_key.as_bytes().as_slice())
            .bind(self.hw_id.as_slice())
    }
    fn row_data(row: &MySqlRow) -> anyhow::Result<GetGameServerGroupKeyPairData> {
        Ok(GetGameServerGroupKeyPairData {
            serialized_key_pair: row.try_get("encrypted_key_pair")?,
        })
    }
}

#[derive(Debug)]
pub struct StoreGameServerGroupKeyPair<'a> {
    pub add_serialized_key_pair: &'a [u8],
    pub add_public: &'a [u8],
    pub public_key: &'a VerifyingKey,
    pub hw_id: &'a [u8; 32],
}

#[async_trait::async_trait]
impl<'a> Query<()> for StoreGameServerGroupKeyPair<'a> {
    async fn prepare_mysql(
        connection: &mut sqlx::MySqlConnection,
    ) -> anyhow::Result<sqlx::mysql::MySqlStatement<'static>> {
        Ok(connection
            .prepare(include_str!("game_server_add_key.sql"))
            .await?)
    }
    fn query_mysql<'b>(
        &'b self,
        statement: &'b sqlx::mysql::MySqlStatement<'static>,
    ) -> sqlx::query::Query<'b, sqlx::MySql, sqlx::mysql::MySqlArguments> {
        statement
            .query()
            .bind(self.add_serialized_key_pair)
            .bind(self.add_public)
            .bind(self.public_key.as_bytes().as_slice())
            .bind(self.hw_id.as_slice())
            .bind(self.add_serialized_key_pair)
            .bind(self.add_public)
    }
    fn row_data(_row: &MySqlRow) -> anyhow::Result<()> {
        Err(anyhow!("Row data is not supported"))
    }
}
