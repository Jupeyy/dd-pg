use sqlx::Acquire;
use sqlx::Executor;
use sqlx::Statement;

pub async fn setup_mysql(pool: &sqlx::MySqlPool) -> anyhow::Result<()> {
    let mut pool_con = pool.acquire().await?;
    let con = pool_con.acquire().await?;

    // first create all statements (syntax check)
    let account = con.prepare(include_str!("setup/account.sql")).await?;
    let login_tokens = con.prepare(include_str!("setup/login_tokens.sql")).await?;
    let reset_codes = con.prepare(include_str!("setup/reset_codes.sql")).await?;
    let session = con.prepare(include_str!("setup/session.sql")).await?;

    // afterwards actually create tables
    account.query().execute(&mut *con).await?;
    login_tokens.query().execute(&mut *con).await?;
    reset_codes.query().execute(&mut *con).await?;
    session.query().execute(&mut *con).await?;

    Ok(())
}

pub async fn delete_mysql(pool: &sqlx::MySqlPool) -> anyhow::Result<()> {
    let mut pool_con = pool.acquire().await?;
    let con = pool_con.acquire().await?;

    // first create all statements (syntax check)
    // delete in reverse order to creating
    let session = con
        .prepare(include_str!("setup/delete/session.sql"))
        .await?;
    let login_tokens = con
        .prepare(include_str!("setup/delete/login_tokens.sql"))
        .await?;
    let reset_codes = con
        .prepare(include_str!("setup/delete/reset_codes.sql"))
        .await?;
    let account = con
        .prepare(include_str!("setup/delete/account.sql"))
        .await?;

    // afterwards actually drop tables
    let session = session.query().execute(&mut *con).await;
    let login_tokens = login_tokens.query().execute(&mut *con).await;
    let reset_codes = reset_codes.query().execute(&mut *con).await;
    let account = account.query().execute(&mut *con).await;

    // handle errors at once
    session.and(login_tokens).and(reset_codes).and(account)?;

    Ok(())
}
