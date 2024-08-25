use std::sync::Arc;

use game_database::{
    statement::{Statement, StatementBuilder},
    traits::DbInterface,
};

#[derive(Clone)]
pub struct SetupRaceV1(Arc<Statement<(), ()>>);

impl SetupRaceV1 {
    pub async fn new(db: Arc<dyn DbInterface>) -> anyhow::Result<Self> {
        let builder = StatementBuilder::<_, (), ()>::mysql(
            "setup_race",
            include_str!("mysql/setup_ddnet/race.sql"),
            |_| vec![],
        );

        Ok(Self(Arc::new(Statement::new(db.clone(), builder).await?)))
    }
}

#[derive(Clone)]
pub struct SetupTeamraceV1(Arc<Statement<(), ()>>);

impl SetupTeamraceV1 {
    pub async fn new(db: Arc<dyn DbInterface>) -> anyhow::Result<Self> {
        let builder = StatementBuilder::<_, (), ()>::mysql(
            "setup_teamrace",
            include_str!("mysql/setup_ddnet/teamrace.sql"),
            |_| vec![],
        );

        Ok(Self(Arc::new(Statement::new(db.clone(), builder).await?)))
    }
}

#[derive(Clone)]
pub struct SetupMapsV1(Arc<Statement<(), ()>>);

impl SetupMapsV1 {
    pub async fn new(db: Arc<dyn DbInterface>) -> anyhow::Result<Self> {
        let builder = StatementBuilder::<_, (), ()>::mysql(
            "setup_maps",
            include_str!("mysql/setup_ddnet/maps.sql"),
            |_| vec![],
        );

        Ok(Self(Arc::new(Statement::new(db.clone(), builder).await?)))
    }
}

#[derive(Clone)]
pub struct SetupSavesV1(Arc<Statement<(), ()>>);

impl SetupSavesV1 {
    pub async fn new(db: Arc<dyn DbInterface>) -> anyhow::Result<Self> {
        let builder = StatementBuilder::<_, (), ()>::mysql(
            "setup_saves",
            include_str!("mysql/setup_ddnet/saves.sql"),
            |_| vec![],
        );

        Ok(Self(Arc::new(Statement::new(db.clone(), builder).await?)))
    }
}

#[derive(Clone)]
pub struct SetupPointsV1(Arc<Statement<(), ()>>);

impl SetupPointsV1 {
    pub async fn new(db: Arc<dyn DbInterface>) -> anyhow::Result<Self> {
        let builder = StatementBuilder::<_, (), ()>::mysql(
            "setup_points",
            include_str!("mysql/setup_ddnet/points.sql"),
            |_| vec![],
        );

        Ok(Self(Arc::new(Statement::new(db.clone(), builder).await?)))
    }
}

// v2

#[derive(Clone)]
pub struct SetupRaceV2(Arc<Statement<(), ()>>);

impl SetupRaceV2 {
    pub async fn new(db: Arc<dyn DbInterface>) -> anyhow::Result<Self> {
        let builder = StatementBuilder::<_, (), ()>::mysql(
            "setup_race_v2",
            include_str!("mysql/setup_ddnet/race_v2.sql"),
            |_| vec![],
        );

        Ok(Self(Arc::new(Statement::new(db.clone(), builder).await?)))
    }
}

#[derive(Clone)]
pub struct SetupTeamraceV2(Arc<Statement<(), ()>>);

impl SetupTeamraceV2 {
    pub async fn new(db: Arc<dyn DbInterface>) -> anyhow::Result<Self> {
        let builder = StatementBuilder::<_, (), ()>::mysql(
            "setup_teamrace_v2",
            include_str!("mysql/setup_ddnet/teamrace_v2.sql"),
            |_| vec![],
        );

        Ok(Self(Arc::new(Statement::new(db.clone(), builder).await?)))
    }
}

pub async fn setup(db: Arc<dyn DbInterface>) -> anyhow::Result<()> {
    let setup_race_v1 = SetupRaceV1::new(db.clone()).await?;
    let setup_teamrace_v1 = SetupTeamraceV1::new(db.clone()).await?;
    let setup_maps_v1 = SetupMapsV1::new(db.clone()).await?;
    let setup_saves_v1 = SetupSavesV1::new(db.clone()).await?;
    let setup_points_v1 = SetupPointsV1::new(db.clone()).await?;

    let setup_race_v2 = SetupRaceV2::new(db.clone()).await?;
    let setup_teamrace_v2 = SetupTeamraceV2::new(db.clone()).await?;

    db.setup(
        "game-server-ddnet",
        vec![
            (
                1,
                vec![
                    setup_race_v1.0.unique_id.clone(),
                    setup_teamrace_v1.0.unique_id.clone(),
                    setup_maps_v1.0.unique_id.clone(),
                    setup_saves_v1.0.unique_id.clone(),
                    setup_points_v1.0.unique_id.clone(),
                ],
            ),
            (
                2,
                vec![
                    setup_race_v2.0.unique_id.clone(),
                    setup_teamrace_v2.0.unique_id.clone(),
                ],
            ),
        ]
        .into_iter()
        .collect(),
    )
    .await
}
