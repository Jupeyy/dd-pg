use std::sync::Arc;

use game_database::{
    statement::{Statement, StatementBuilder},
    traits::{DbInterface, DbKind, DbKindExtra},
};

#[derive(Clone)]
pub struct SetupSaves {
    stmts: Arc<Vec<Arc<Statement<(), ()>>>>,
}

impl SetupSaves {
    pub async fn new(db: Arc<dyn DbInterface>) -> anyhow::Result<Self> {
        let mut stmts = Vec::new();
        let kinds = db.kinds();

        if kinds.contains(&DbKind::MySql(DbKindExtra::Main)) {
            let builder = StatementBuilder::<_, (), ()>::new(
                DbKind::MySql(DbKindExtra::Main),
                include_str!("mysql/save/saves.sql"),
                |_| vec![],
            );
            let stmt = Arc::new(Statement::new(db.clone(), builder).await?);
            stmts.push(stmt.clone());
        }
        if kinds.contains(&DbKind::Sqlite(DbKindExtra::Main)) {
            let builder = StatementBuilder::<_, (), ()>::new(
                DbKind::Sqlite(DbKindExtra::Main),
                include_str!("sqlite/save/saves.sql"),
                |_| vec![],
            );
            let stmt = Arc::new(Statement::new(db.clone(), builder).await?);
            stmts.push(stmt.clone());
        }

        Ok(Self {
            stmts: Arc::new(stmts),
        })
    }
}

pub async fn setup(db: Arc<dyn DbInterface>) -> anyhow::Result<()> {
    let setup_saves = SetupSaves::new(db.clone()).await?;

    db.setup(
        "game-server-vanilla",
        vec![(1, setup_saves.stmts.iter().map(|s| s.unique_id).collect())]
            .into_iter()
            .collect(),
    )
    .await
}
