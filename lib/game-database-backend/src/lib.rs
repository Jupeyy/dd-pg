use std::{
    collections::{BTreeMap, HashMap, HashSet},
    sync::Arc,
};

use account_sql::version::{get_version, set_version};
use anyhow::anyhow;
use game_database::{
    statement::{QueryProperties, StatementDriverProps},
    traits::{DbInterface, DbKind},
    types::{DbType, UnixTimestamp},
};
use parking_lot::Mutex;
use sql::database::Database;
use sqlx::{
    any::{AnyArguments, AnyRow, AnyStatement},
    query::Query,
    Acquire, Any, Connection, Executor, Row, Statement,
};

#[derive(Clone)]
pub struct CachedStatement {
    stmt: Arc<AnyStatement<'static>>,
    qry_props: QueryProperties,
}

pub struct GameDbBackend {
    db: Arc<Database>,
    statements: Mutex<HashMap<String, CachedStatement>>,
}

impl GameDbBackend {
    pub fn new(db: Arc<Database>) -> anyhow::Result<Self> {
        Ok(Self {
            db,
            statements: Default::default(),
        })
    }

    fn get_query<'a>(
        &self,
        stmt: &'a CachedStatement,
        args: &'a [DbType],
    ) -> anyhow::Result<Query<'a, Any, AnyArguments<'a>>> {
        let mut qry = stmt.stmt.query();
        for arg in args.iter() {
            match arg {
                DbType::I16(v) => qry = qry.bind(*v),
                DbType::I32(v) => qry = qry.bind(*v),
                DbType::I64(v) => qry = qry.bind(*v),
                DbType::F32(v) => qry = qry.bind(*v),
                DbType::F64(v) => qry = qry.bind(*v),
                DbType::Bool(v) => qry = qry.bind(*v),
                DbType::String(v) => qry = qry.bind(v),
                DbType::Vec(v) => qry = qry.bind(v),
                DbType::DateTime(v) => {
                    let time_stamp =
                        <sqlx::types::chrono::DateTime<sqlx::types::chrono::Utc>>::from_timestamp(
                            v.secs as i64,
                            v.subsec_nanos,
                        )
                        .ok_or_else(|| anyhow!("not a valid utc timestamp"))?;
                    qry = qry.bind(time_stamp);
                }
            }
        }

        Ok(qry)
    }

    fn get_result(
        stmt: &CachedStatement,
        rows: impl Iterator<Item = AnyRow>,
    ) -> anyhow::Result<Vec<HashMap<String, DbType>>> {
        rows.map(|row| {
            stmt.qry_props
                .result_mapping
                .iter()
                .map(|(name, ty)| {
                    let val = match ty {
                        DbType::I16(_) => DbType::I16(row.try_get::<i16, _>(name.as_str())?),
                        DbType::I32(_) => DbType::I32(row.try_get::<i32, _>(name.as_str())?),
                        DbType::I64(_) => DbType::I64(row.try_get::<i64, _>(name.as_str())?),
                        DbType::F32(_) => DbType::F32(row.try_get::<f32, _>(name.as_str())?),
                        DbType::F64(_) => DbType::F64(row.try_get::<f64, _>(name.as_str())?),
                        DbType::Bool(_) => DbType::Bool(row.try_get::<bool, _>(name.as_str())?),
                        DbType::String(_) => {
                            DbType::String(row.try_get::<String, _>(name.as_str())?)
                        }
                        DbType::Vec(_) => DbType::Vec(row.try_get::<Vec<u8>, _>(name.as_str())?),
                        DbType::DateTime(_) => {
                            let time_stamp: sqlx::types::chrono::DateTime<
                                sqlx::types::chrono::Utc,
                            > = row.try_get(name.as_str())?;
                            DbType::DateTime(UnixTimestamp {
                                secs: time_stamp.timestamp() as u64,
                                subsec_nanos: time_stamp.timestamp_subsec_nanos(),
                            })
                        }
                    };
                    anyhow::Ok((name.clone(), val))
                })
                .collect::<anyhow::Result<HashMap<_, _>>>()
        })
        .collect::<anyhow::Result<Vec<_>>>()
    }
}

#[async_trait::async_trait]
impl DbInterface for GameDbBackend {
    fn kinds(&self) -> HashSet<DbKind> {
        let mut res: HashSet<DbKind> = Default::default();
        res.insert(DbKind::MySql);
        res
    }

    async fn setup(
        &self,
        version_name: &str,
        versioned_stmts: BTreeMap<i64, Vec<String>>,
    ) -> anyhow::Result<()> {
        let pool = &self.db.pool;

        let mut connection = pool.acquire().await?;
        let connection = connection.acquire().await?;

        let versioned_stmts = {
            let prepared_stmts = self.statements.lock();
            versioned_stmts
                .into_iter()
                .map(|(version, stmts)| {
                    stmts
                        .into_iter()
                        .map(|stmt| prepared_stmts.get(&stmt).map(|s| s.stmt.clone()))
                        .collect::<Option<Vec<_>>>()
                        .map(|stmts| (version, stmts))
                })
                .collect::<Option<Vec<_>>>()
                .ok_or_else(|| anyhow!("at least one of the statements was not prepared."))?
        };

        let version_name = version_name.to_string();
        connection
            .transaction(|con| {
                Box::pin(async move {
                    let mut version = get_version(con, &version_name).await?;
                    for (stmts_version, stmts) in versioned_stmts {
                        if version < stmts_version {
                            for s in stmts {
                                con.execute(s.query()).await?;
                            }

                            set_version(con, &version_name, stmts_version).await?;
                            version = stmts_version;
                        }
                    }
                    anyhow::Ok(())
                })
            })
            .await?;

        Ok(())
    }

    async fn prepare_statement(
        &self,
        unique_id: &str,
        query_props: &QueryProperties,
        driver_props: &HashMap<DbKind, StatementDriverProps>,
    ) -> anyhow::Result<()> {
        let pool = &self.db.pool;
        let mut connection = pool.acquire().await?;
        let connection = connection.acquire().await?;

        let stm = driver_props
            .get(&DbKind::MySql)
            .ok_or_else(|| anyhow!("MySQL backend not supported by statement"))?;
        let stm = connection.prepare(&stm.sql).await?;

        self.statements.lock().insert(
            unique_id.to_string(),
            CachedStatement {
                stmt: Arc::new(stm.to_owned()),
                qry_props: query_props.clone(),
            },
        );
        Ok(())
    }

    fn drop_statement(&self, unique_id: &str) {
        self.statements.lock().remove(unique_id);
    }

    async fn fetch_optional(
        &self,
        unique_id: &str,
        args: Vec<DbType>,
    ) -> anyhow::Result<Option<HashMap<String, DbType>>> {
        let pool = &self.db.pool;
        let mut connection = pool.acquire().await?;
        let connection = connection.acquire().await?;

        let stmt = self
            .statements
            .lock()
            .get(unique_id)
            .cloned()
            .ok_or_else(|| anyhow!("no statement with id \"{}\" found", unique_id))?;
        let qry = self.get_query(&stmt, &args)?;

        connection
            .fetch_optional(qry)
            .await?
            .map(|res| {
                Self::get_result(&stmt, [res].into_iter()).and_then(|res| {
                    res.into_iter()
                        .next()
                        .ok_or_else(|| anyhow!("no result fetched"))
                })
            })
            .transpose()
    }

    async fn fetch_one(
        &self,
        unique_id: &str,
        args: Vec<DbType>,
    ) -> anyhow::Result<HashMap<String, DbType>> {
        let pool = &self.db.pool;
        let mut connection = pool.acquire().await?;
        let connection = connection.acquire().await?;

        let stmt = self
            .statements
            .lock()
            .get(unique_id)
            .cloned()
            .ok_or_else(|| anyhow!("no statement with id \"{}\" found", unique_id))?;
        let qry = self.get_query(&stmt, &args)?;

        Self::get_result(&stmt, [connection.fetch_one(qry).await?].into_iter()).and_then(|res| {
            res.into_iter()
                .next()
                .ok_or_else(|| anyhow!("no result fetched"))
        })
    }

    async fn fetch_all(
        &self,
        unique_id: &str,
        args: Vec<DbType>,
    ) -> anyhow::Result<Vec<HashMap<String, DbType>>> {
        let pool = &self.db.pool;
        let mut connection = pool.acquire().await?;
        let connection = connection.acquire().await?;

        let stmt = self
            .statements
            .lock()
            .get(unique_id)
            .cloned()
            .ok_or_else(|| anyhow!("no statement with id \"{}\" found", unique_id))?;
        let qry = self.get_query(&stmt, &args)?;

        Self::get_result(&stmt, connection.fetch_all(qry).await?.into_iter())
    }
}

#[cfg(test)]
mod test {
    use std::sync::Arc;

    use game_database::{
        statement::{Statement, StatementBuilder},
        StatementArgs, StatementResult,
    };
    use sql::database::{Database, DatabaseDetails};

    use crate::GameDbBackend;

    #[tokio::test]
    async fn builder() -> anyhow::Result<()> {
        #[derive(StatementArgs)]
        struct StatementArg {
            map: String,
            server: String,
            offset: i64,
            count: i64,
        }

        #[derive(Debug, StatementResult)]
        struct StatementResult {
            name: String,
            time: f32,
            ranking: i64,
        }

        let builder = StatementBuilder::<_, StatementArg, StatementResult>::mysql(
            "top",
            "
            SELECT 
                name, time, ranking 
            FROM 
                ( 
                    SELECT 
                        RANK() OVER w AS ranking, 
                        MIN(Time) AS time, 
                        Name as name 
                    FROM 
                        record_race 
                    WHERE 
                        Map = ? AND 
                        Server LIKE ? 
                    GROUP BY Name 
                    WINDOW w AS (ORDER BY MIN(Time)) 
                ) as a 
            ORDER BY Ranking DESC 
            LIMIT ? OFFSET ?
            ;",
            |arg| vec![arg.map, arg.server, arg.count, arg.offset],
        );

        let db = Arc::new(GameDbBackend::new(Arc::new(
            Database::new(DatabaseDetails {
                host: "localhost".into(),
                port: 3306,
                database: "teeworlds".into(),
                username: "ddnet-account-test".into(),
                password: "test".into(),
                ca_cert_path: "/etc/mysql/ssl/ca-cert.pem".into(),
                connection_count: 3,
            })
            .await?,
        ))?);

        let b = base::benchmark::Benchmark::new(true);
        let stm = Statement::new(db.clone(), builder).await?;
        b.bench("statement");

        let res = stm
            .fetch_all(StatementArg {
                map: "Multeasymap".into(),
                server: "%".into(),
                offset: 5,
                count: 10,
            })
            .await?;
        b.bench("result");

        dbg!(res);

        let builder = StatementBuilder::<_, (), StatementRankResult>::mysql(
            "rank", "
            SELECT Ranking as ranking, Time as time, PercentRank as rank_percent 
            FROM (  
                SELECT 
                    RANK() OVER w AS Ranking, PERCENT_RANK() OVER w as PercentRank, MIN(Time) AS Time, Name 
                FROM 
                    record_race 
                WHERE 
                    Map = 'Multeasymap' AND Server LIKE '%' 
                GROUP BY 
                    Name 
                WINDOW w AS (ORDER BY MIN(Time))
            ) as a 
            WHERE Name = 'deen';
        ",|_| vec![]);
        let stmt = Statement::new(db, builder).await?;
        b.bench("statement");

        #[derive(Debug, StatementResult)]
        struct StatementRankResult {
            ranking: i64,
            time: f32,
            rank_percent: f32,
        }

        let res = stmt.fetch_all(()).await?;
        b.bench("result");

        dbg!(res);

        Ok(())
    }
}
