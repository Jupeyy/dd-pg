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
    kind: DbKind,
    props: StatementDriverProps,
    query_props: QueryProperties,

    _arg_indices: PhantomData<AI>,
    _args: PhantomData<A>,
    _res: PhantomData<R>,
}

impl<AI, A: DbStatementArgIndexInterface<AI>, R: DbStatementResultInterface>
    StatementBuilder<AI, A, R>
{
    pub fn new(kind: DbKind, sql: &str, arguments_mapping: impl FnOnce(AI) -> Vec<usize>) -> Self {
        let props = StatementDriverProps {
            sql: sql.to_string(),
            arguments_mapping: arguments_mapping(A::arg_indices()),
        };
        Self {
            kind,
            props,
            query_props: QueryProperties {
                result_mapping: R::mapping(),
            },

            _arg_indices: PhantomData,
            _args: PhantomData,
            _res: PhantomData,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatementProperties {
    props: StatementDriverProps,
    pub query: QueryProperties,
}

pub struct Statement<A: DbStatementArgInterface, R: DbStatementResultInterface> {
    db: Arc<dyn DbInterface>,
    inner: StatementProperties,
    pub unique_id: u64,

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
        let unique_id = db
            .prepare_statement(&builder.query_props, &builder.kind, &builder.props)
            .await?;

        Ok(Self {
            db,
            inner: StatementProperties {
                props: builder.props,
                query: builder.query_props,
            },
            unique_id,

            _args: Default::default(),
            _result: Default::default(),
        })
    }

    pub async fn fetch_optional(&self, args: A) -> anyhow::Result<Option<R>> {
        let db_args = args.to_db_args();
        let res = self
            .db
            .fetch_optional(
                self.unique_id,
                self.inner
                    .props
                    .arguments_mapping
                    .iter()
                    .map(|arg| db_args.get(*arg).cloned())
                    .collect::<Option<Vec<_>>>()
                    .ok_or_else(|| anyhow!("argument index was out of bounds."))?,
            )
            .await?;

        res.map(|res| R::new(res)).transpose()
    }

    pub async fn fetch_one(&self, args: A) -> anyhow::Result<R> {
        let db_args = args.to_db_args();
        let res = self
            .db
            .fetch_one(
                self.unique_id,
                self.inner
                    .props
                    .arguments_mapping
                    .iter()
                    .map(|arg| db_args.get(*arg).cloned())
                    .collect::<Option<Vec<_>>>()
                    .ok_or_else(|| anyhow!("argument index was out of bounds."))?,
            )
            .await?;

        R::new(res)
    }

    pub async fn fetch_all(&self, args: A) -> anyhow::Result<Vec<R>> {
        let db_args = args.to_db_args();
        let res = self
            .db
            .fetch_all(
                self.unique_id,
                self.inner
                    .props
                    .arguments_mapping
                    .iter()
                    .map(|arg| db_args.get(*arg).cloned())
                    .collect::<Option<Vec<_>>>()
                    .ok_or_else(|| anyhow!("argument index was out of bounds."))?,
            )
            .await?;

        res.into_iter().map(|res| R::new(res)).collect()
    }

    /// Returns the affected rows count
    pub async fn execute(&self, args: A) -> anyhow::Result<u64> {
        let db_args = args.to_db_args();
        self.db
            .execute(
                self.unique_id,
                self.inner
                    .props
                    .arguments_mapping
                    .iter()
                    .map(|arg| db_args.get(*arg).cloned())
                    .collect::<Option<Vec<_>>>()
                    .ok_or_else(|| anyhow!("argument index was out of bounds."))?,
            )
            .await
    }
}

impl<A: DbStatementArgInterface, R: DbStatementResultInterface> Drop for Statement<A, R> {
    fn drop(&mut self) {
        self.db.drop_statement(self.unique_id);
    }
}
