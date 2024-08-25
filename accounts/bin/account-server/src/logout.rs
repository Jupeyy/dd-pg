pub mod queries;

use std::sync::Arc;

use account_sql::query::Query;
use accounts_shared::client::logout::LogoutRequest;
use axum::{response, Json};
use chrono::TimeDelta;
use sqlx::{Acquire, AnyPool};

use crate::{internal_err::InternalErr, shared::Shared};

use self::queries::RemoveSession;

pub async fn logout_request(
    shared: Arc<Shared>,
    pool: AnyPool,
    Json(data): Json<LogoutRequest>,
) -> response::Result<Json<()>> {
    logout(shared, pool, data)
        .await
        .map_err(|err| InternalErr(("logout".into(), err)).into())
        .map(Json)
}

pub async fn logout(shared: Arc<Shared>, pool: AnyPool, data: LogoutRequest) -> anyhow::Result<()> {
    data.account_data
        .public_key
        .verify_strict(data.time_stamp.to_string().as_bytes(), &data.signature)?;
    let now = chrono::Utc::now();
    let delta = now.signed_duration_since(data.time_stamp);
    anyhow::ensure!(
        delta < TimeDelta::seconds(20) && delta > TimeDelta::seconds(-20),
        "time stamp was not in a valid time frame."
    );

    let mut connection = pool.acquire().await?;
    let connection = connection.acquire().await?;

    // remove this session
    let qry = RemoveSession {
        pub_key: data.account_data.public_key.as_bytes(),
        hw_id: &data.account_data.hw_id,
    };

    qry.query(&shared.db.logout_statement)
        .execute(connection)
        .await?;

    Ok(())
}
