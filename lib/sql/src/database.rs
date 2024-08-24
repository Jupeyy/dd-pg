use std::future::Future;

use base_io::io_batcher::{IoBatcher, IoBatcherTask};
use sqlx::{
    any::{AnyArguments, AnyPoolOptions, AnyRow},
    mysql::MySqlConnectOptions,
    query::QueryAs,
    Any, FromRow, Pool,
};

#[derive(Debug, Clone)]
pub struct DatabaseDetails {
    pub host: String,
    pub port: u16,
    pub database: String,
    pub username: String,
    pub password: String,
    pub ca_cert_path: String,
    pub connection_count: usize,
}

#[derive(Debug)]
pub struct Database {
    _details: DatabaseDetails,
    pub pool: Pool<Any>,
}

impl Database {
    pub async fn new(connection_details: DatabaseDetails) -> anyhow::Result<Self> {
        let pool = AnyPoolOptions::new()
            .max_connections(connection_details.connection_count as u32)
            .connect_with(
                MySqlConnectOptions::new()
                    .charset("utf8mb4")
                    .host(&connection_details.host)
                    .port(connection_details.port)
                    .database(&connection_details.database)
                    .username(&connection_details.username)
                    .password(&connection_details.password)
                    .ssl_mode(sqlx::mysql::MySqlSslMode::Required)
                    .ssl_ca(&connection_details.ca_cert_path)
                    .into(),
            )
            .await?;

        Ok(Self {
            _details: connection_details,
            pool,
        })
    }

    pub fn get_query<'a, F>(str: &'a str) -> QueryAs<'a, Any, F, AnyArguments>
    where
        F: for<'r> FromRow<'r, AnyRow>,
    {
        sqlx::query_as::<_, F>(str)
    }
}
