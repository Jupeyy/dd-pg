use sqlx::Acquire;
use sqlx::Executor;
use sqlx::Statement;

/// Sets up all mysql tables required for a game server user
pub async fn setup_mysql(pool: &sqlx::MySqlPool) -> anyhow::Result<()> {
    let mut pool_con = pool.acquire().await?;
    let con = pool_con.acquire().await?;

    // first create all statements (syntax check)
    let user = con.prepare(include_str!("setup/user.sql")).await?;

    // afterwards actually create tables
    user.query().execute(&mut *con).await?;

    Ok(())
}

/// Drop all tables related to a game server mysql setup
pub async fn delete_mysql(pool: &sqlx::MySqlPool) -> anyhow::Result<()> {
    let mut pool_con = pool.acquire().await?;
    let con = pool_con.acquire().await?;

    // first create all statements (syntax check)
    // delete in reverse order to creating
    let user = con.prepare(include_str!("setup/delete/user.sql")).await?;

    // afterwards actually drop tables
    let user = user.query().execute(&mut *con).await;

    // handle errors at once
    user?;

    Ok(())
}
