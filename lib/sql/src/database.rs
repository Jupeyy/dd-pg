use std::future::Future;

use base_io::io_batcher::{IoBatcher, IoBatcherTask};
use sqlx::{
    mysql::{MySqlArguments, MySqlConnectOptions, MySqlPoolOptions, MySqlRow},
    query::QueryAs,
    FromRow, MySql, Pool,
};

#[derive(Debug, Clone)]
pub struct DatabaseDetails {
    pub host: String,
    pub port: u16,
    pub database: String,
    pub username: String,
    pub password: String,
    pub thread_count: usize,
}

pub struct Database {
    _details: DatabaseDetails,
    io_batcher: IoBatcher,
    pub pool: Pool<MySql>,
}

impl Database {
    pub fn new(connection_details: DatabaseDetails) -> anyhow::Result<Self> {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .worker_threads(connection_details.thread_count)
            .max_blocking_threads(2) // at least two
            .build()
            .unwrap();

        let pool = rt.block_on(async {
            anyhow::Ok(
                MySqlPoolOptions::new()
                    .max_connections(connection_details.thread_count as u32)
                    .connect_with(
                        MySqlConnectOptions::new()
                            .charset("utf8mb4")
                            .host(&connection_details.host)
                            .port(connection_details.port)
                            .database(&connection_details.database)
                            .username(&connection_details.username)
                            .password(&connection_details.password),
                    )
                    .await?,
            )
        })?;

        let io_batcher = IoBatcher::new(rt);

        Ok(Self {
            _details: connection_details,
            io_batcher,
            pool,
        })
    }

    pub fn get_query<'a, F>(str: &'a str) -> QueryAs<'a, MySql, F, MySqlArguments>
    where
        F: for<'r> FromRow<'r, MySqlRow>,
    {
        sqlx::query_as::<_, F>(str)
    }

    pub fn queue_task<S, F>(&mut self, task: F) -> IoBatcherTask<S>
    where
        S: Send + Sync + 'static,
        F: Future<Output = anyhow::Result<S>> + Send + 'static,
    {
        self.io_batcher.spawn(task)
    }
}
