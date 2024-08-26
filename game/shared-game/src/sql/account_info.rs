use std::sync::Arc;

use accounts_types::account_id::AccountId;
use game_database::{
    statement::{Statement, StatementBuilder},
    traits::DbInterface,
    types::UnixTimestamp,
    StatementArgs, StatementResult,
};

#[derive(Debug, StatementArgs)]
struct StatementArg {
    account_id: AccountId,
}

#[derive(Debug, StatementResult)]
pub struct StatementResult {
    pub id: i64,
    pub name: String,
    pub create_time: UnixTimestamp,
}

#[derive(Clone)]
pub struct AccountInfo(Arc<Statement<StatementArg, StatementResult>>);

impl AccountInfo {
    pub async fn new(db: Arc<dyn DbInterface>) -> anyhow::Result<Self> {
        let builder = StatementBuilder::<_, StatementArg, StatementResult>::mysql(
            "account_info",
            include_str!("mysql/account_info/account_info.sql"),
            |arg| vec![arg.account_id],
        );

        let stm = Arc::new(Statement::new(db.clone(), builder).await?);

        Ok(Self(stm))
    }

    pub async fn fetch(&self, account_id: AccountId) -> anyhow::Result<StatementResult> {
        self.0.fetch_one(StatementArg { account_id }).await
    }
}
