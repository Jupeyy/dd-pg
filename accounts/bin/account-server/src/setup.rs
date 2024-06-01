use sqlx::Acquire;
use sqlx::Executor;
use sqlx::Statement;

pub async fn setup_mysql(pool: &sqlx::MySqlPool) -> anyhow::Result<()> {
    let mut pool_con = pool.acquire().await?;
    let con = pool_con.acquire().await?;

    // first create all statements (syntax check)
    let account = con.prepare(include_str!("setup/account.sql")).await?;
    let account_keys = con.prepare(include_str!("setup/account_keys.sql")).await?;
    let account_game_server_key = con
        .prepare(include_str!("setup/account_game_server_key.sql"))
        .await?;
    let verify_tokens = con.prepare(include_str!("setup/verify_tokens.sql")).await?;
    let reset_codes = con.prepare(include_str!("setup/reset_codes.sql")).await?;
    let register_tokens = con
        .prepare(include_str!("setup/register_tokens.sql"))
        .await?;
    let otps = con.prepare(include_str!("setup/otps.sql")).await?;
    let session = con.prepare(include_str!("setup/session.sql")).await?;

    // afterwards actually create tables
    account.query().execute(&mut *con).await?;
    account_keys.query().execute(&mut *con).await?;
    account_game_server_key.query().execute(&mut *con).await?;
    verify_tokens.query().execute(&mut *con).await?;
    reset_codes.query().execute(&mut *con).await?;
    register_tokens.query().execute(&mut *con).await?;
    otps.query().execute(&mut *con).await?;
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
    let verify_tokens = con
        .prepare(include_str!("setup/delete/verify_tokens.sql"))
        .await?;
    let reset_codes = con
        .prepare(include_str!("setup/delete/reset_codes.sql"))
        .await?;
    let register_tokens = con
        .prepare(include_str!("setup/delete/register_tokens.sql"))
        .await?;
    let otps = con.prepare(include_str!("setup/delete/otps.sql")).await?;
    let account_keys = con
        .prepare(include_str!("setup/delete/account_keys.sql"))
        .await?;
    let account_game_server_key = con
        .prepare(include_str!("setup/delete/account_game_server_key.sql"))
        .await?;
    let account = con
        .prepare(include_str!("setup/delete/account.sql"))
        .await?;

    // afterwards actually drop tables
    let session = session.query().execute(&mut *con).await;
    let verify_tokens = verify_tokens.query().execute(&mut *con).await;
    let reset_codes = reset_codes.query().execute(&mut *con).await;
    let register_tokens = register_tokens.query().execute(&mut *con).await;
    let otps = otps.query().execute(&mut *con).await;
    let account_keys = account_keys.query().execute(&mut *con).await;
    let account_game_server_key = account_game_server_key.query().execute(&mut *con).await;
    let account = account.query().execute(&mut *con).await;

    // handle errors at once
    session
        .and(verify_tokens)
        .and(reset_codes)
        .and(register_tokens)
        .and(otps)
        .and(account_keys)
        .and(account_game_server_key)
        .and(account)?;

    Ok(())
}
