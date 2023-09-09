use std::{future::Future, sync::Arc};

use base_fs::io_batcher::{TokIOBatcher, TokIOBatcherTask};
use sqlx::{
    mysql::{MySqlArguments, MySqlConnectOptions, MySqlRow},
    query::QueryAs,
    Connection, FromRow, MySql, MySqlConnection,
};
use tokio::{sync::Mutex, task::JoinHandle};

#[derive(Debug, Clone)]
pub struct DatabaseDetails {
    pub host: String,
    pub port: u16,
    pub database: String,
    pub username: String,
    pub password: String,
    pub thread_count: usize,
}

#[derive(Debug, Clone)]
pub struct DBConnection {
    con: Arc<Mutex<Option<MySqlConnection>>>,

    details: Arc<DatabaseDetails>,
}

impl DBConnection {
    pub async fn from_query_single<F>(
        &mut self,
        query: QueryAs<'_, MySql, F, MySqlArguments>,
    ) -> anyhow::Result<F>
    where
        F: Send + Unpin + for<'r> FromRow<'r, MySqlRow>,
    {
        let mut connection = self.con.lock().await;
        let con = Database::get_connection(&mut connection, &self.details).await;
        Ok(query.fetch_one(con).await?)
    }

    pub async fn from_query<F>(
        &mut self,
        query: QueryAs<'_, MySql, F, MySqlArguments>,
    ) -> anyhow::Result<Vec<F>>
    where
        F: Send + Unpin + for<'r> FromRow<'r, MySqlRow>,
    {
        let mut connection = self.con.lock().await;
        let con = Database::get_connection(&mut connection, &self.details).await;
        Ok(query.fetch_all(con).await?)
    }
}

pub struct Database {
    _details: DatabaseDetails,
    io_batcher: TokIOBatcher,
    /// use get_connection
    pub connection: DBConnection,
}

impl Database {
    pub fn new(connection_details: DatabaseDetails) -> Self {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .worker_threads(connection_details.thread_count)
            .max_blocking_threads(2) // at least two
            .build()
            .unwrap();
        let io_batcher = TokIOBatcher::new(rt);

        let connection = DBConnection {
            con: Arc::new(Mutex::new(None)),
            details: Arc::new(connection_details.clone()),
        };

        Self {
            _details: connection_details,
            io_batcher,
            connection,
        }
    }

    async fn get_connection<'a>(
        connection: &'a mut Option<MySqlConnection>,
        details: &DatabaseDetails,
    ) -> &'a mut MySqlConnection {
        if connection.is_none() {
            let opt = MySqlConnectOptions::new()
                .charset("utf8mb4")
                .host(&details.host)
                .port(details.port)
                .database(&details.database)
                .username(&details.username)
                .password(&details.password);
            if let Ok(con) = sqlx::MySqlConnection::connect_with(&opt).await {
                *connection = Some(con);
                connection.as_mut().unwrap()
            } else {
                todo!()
            }
        } else {
            connection.as_mut().unwrap()
        }
    }

    pub fn get_query<'a, F>(str: &'a str) -> QueryAs<'a, MySql, F, MySqlArguments>
    where
        F: for<'r> FromRow<'r, MySqlRow>,
    {
        sqlx::query_as::<_, F>(str)
    }

    pub fn queue_task<S, F>(&mut self, task: F) -> (TokIOBatcherTask<S>, JoinHandle<()>)
    where
        S: Send + Sync + 'static,
        F: Future<Output = anyhow::Result<S>> + Send + 'static,
    {
        self.io_batcher.spawn_without_queue(task)
    }
}
