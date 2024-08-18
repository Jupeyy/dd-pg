use std::{collections::HashMap, marker::PhantomData, sync::Arc};

use anyhow::anyhow;
use serde::{Deserialize, Serialize};

use crate::{
    traits::{
        DbInterface, DbKind, DbStatementArgIndexInterface, DbStatementArgInterface,
        DbStatementResultInterface,
    },
    types::DbType,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatementDriverProps {
    pub sql: String,
    pub arguments_mapping: Vec<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FetchMode {
    FetchOne,
    FetchAll,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryProperties {
    pub result_mapping: HashMap<String, DbType>,
}

#[derive(Debug)]
pub struct StatementBuilder<AI, A: DbStatementArgIndexInterface<AI>, R> {
    mysql: Option<StatementDriverProps>,
    query_props: QueryProperties,
    unique_id: String,

    _arg_indices: PhantomData<AI>,
    _args: PhantomData<A>,
    _res: PhantomData<R>,
}

impl<AI, A: DbStatementArgIndexInterface<AI>, R: DbStatementResultInterface>
    StatementBuilder<AI, A, R>
{
    pub fn mysql(
        unique_id: &str,
        sql: &str,
        arguments_mapping: impl FnOnce(AI) -> Vec<usize>,
    ) -> Self {
        Self {
            mysql: Some(StatementDriverProps {
                sql: sql.to_string(),
                arguments_mapping: arguments_mapping(A::arg_indices()),
            }),
            query_props: QueryProperties {
                result_mapping: R::mapping(),
            },
            unique_id: unique_id.to_string(),

            _arg_indices: PhantomData,
            _args: PhantomData,
            _res: PhantomData,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatementProperties {
    pub mysql: Option<StatementDriverProps>,
    pub query: QueryProperties,
}

pub struct Statement<A: DbStatementArgInterface, R: DbStatementResultInterface> {
    db: Arc<dyn DbInterface>,
    inner: StatementProperties,
    pub unique_id: String,

    _args: PhantomData<A>,
    _result: PhantomData<R>,
}

impl<A: DbStatementArgInterface, R: DbStatementResultInterface> Statement<A, R> {
    pub async fn new<AI>(
        db: Arc<dyn DbInterface>,
        builder: StatementBuilder<AI, A, R>,
    ) -> anyhow::Result<Self>
    where
        A: DbStatementArgIndexInterface<AI>,
    {
        let mut props: HashMap<DbKind, StatementDriverProps> = Default::default();

        let mut prep = |kind: DbKind, prop: &Option<StatementDriverProps>| {
            if let Some(prop) = prop {
                props.insert(kind, prop.clone());
            }
        };

        prep(DbKind::MySql, &builder.mysql);
        db.prepare_statement(&builder.unique_id, &builder.query_props, &props)
            .await?;

        Ok(Self {
            db,
            inner: StatementProperties {
                mysql: builder.mysql,
                query: builder.query_props,
            },
            unique_id: builder.unique_id,

            _args: Default::default(),
            _result: Default::default(),
        })
    }

    pub async fn fetch_optional(&self, args: A) -> anyhow::Result<Option<R>> {
        if let Some(mysql) = &self.inner.mysql {
            let db_args = args.to_db_args();
            let res = self
                .db
                .fetch_optional(
                    &self.unique_id,
                    mysql
                        .arguments_mapping
                        .iter()
                        .map(|arg| db_args.get(*arg).cloned())
                        .collect::<Option<Vec<_>>>()
                        .ok_or_else(|| anyhow!("argument index was out of bounds."))?,
                )
                .await?;

            res.map(|res| R::new(res)).transpose()
        } else {
            Err(anyhow!("No compatible driver found."))
        }
    }

    pub async fn fetch_one(&self, args: A) -> anyhow::Result<R> {
        if let Some(mysql) = &self.inner.mysql {
            let db_args = args.to_db_args();
            let res = self
                .db
                .fetch_one(
                    &self.unique_id,
                    mysql
                        .arguments_mapping
                        .iter()
                        .map(|arg| db_args.get(*arg).cloned())
                        .collect::<Option<Vec<_>>>()
                        .ok_or_else(|| anyhow!("argument index was out of bounds."))?,
                )
                .await?;

            R::new(res)
        } else {
            Err(anyhow!("No compatible driver found."))
        }
    }

    pub async fn fetch_all(&self, args: A) -> anyhow::Result<Vec<R>> {
        if let Some(mysql) = &self.inner.mysql {
            let db_args = args.to_db_args();
            let res = self
                .db
                .fetch_all(
                    &self.unique_id,
                    mysql
                        .arguments_mapping
                        .iter()
                        .map(|arg| db_args.get(*arg).cloned())
                        .collect::<Option<Vec<_>>>()
                        .ok_or_else(|| anyhow!("argument index was out of bounds."))?,
                )
                .await?;

            res.into_iter().map(|res| R::new(res)).collect()
        } else {
            Err(anyhow!("No compatible driver found."))
        }
    }
}

impl<A: DbStatementArgInterface, R: DbStatementResultInterface> Drop for Statement<A, R> {
    fn drop(&mut self) {
        self.db.drop_statement(&self.unique_id);
    }
}
