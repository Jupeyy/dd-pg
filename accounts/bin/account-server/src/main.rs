//! This is the http + mysql implementation for the account server.

#![deny(missing_docs)]
#![deny(warnings)]
#![deny(clippy::nursery)]
#![deny(clippy::all)]

pub(crate) mod email;
pub(crate) mod internal_err;
pub(crate) mod login_token_email;
pub(crate) mod mysql;
pub(crate) mod setup;
pub(crate) mod shared;
pub(crate) mod sign;

pub(crate) mod login;
#[cfg(test)]
mod tests;

use account_sql::query::Query;
use axum::{Json, Router};
use clap::{command, parser::ValueSource, Arg, ArgAction};
use email::EmailShared;
use login::{
    login_request,
    queries::{CreateSession, InvalidateLoginToken, LoginQry, LoginTokenQry, TryCreateAccount},
};
use login_token_email::{login_token_email, queries::AddLoginToken};
use mysql::MySqlConnectionShared;
use p256::ecdsa::SigningKey;
use serde::{Deserialize, Serialize};
use setup::{delete_mysql, setup_mysql};
use shared::Shared;
use sign::{queries::AuthAttempt, sign_request};
use sqlx::{
    mysql::{MySqlConnectOptions, MySqlPoolOptions},
    MySql, Pool,
};
use std::sync::Arc;
use tokio::net::{TcpListener, TcpSocket};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DbDetails {
    host: String,
    port: u16,
    database: String,
    username: String,
    password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct HttpServerDetails {
    port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct EmailDetails {
    relay: String,
    relay_port: u16,
    username: String,
    password: String,
    /// The name of the sender of all emails
    /// e.g. `accounts@mydomain.org`
    email_from: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AccountServerDetails {
    signing_key: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Details {
    db: DbDetails,
    http: HttpServerDetails,
    email: EmailDetails,
    account_server: AccountServerDetails,
}

pub(crate) async fn prepare_db(details: &DbDetails) -> anyhow::Result<Pool<MySql>> {
    Ok(MySqlPoolOptions::new()
        .max_connections(200)
        .connect_with(
            MySqlConnectOptions::new()
                .charset("utf8mb4")
                .host(&details.host)
                .port(details.port)
                .database(&details.database)
                .username(&details.username)
                .password(&details.password),
        )
        .await?)
}

pub(crate) async fn prepare_mysql(pool: &Pool<MySql>) -> anyhow::Result<MySqlConnectionShared> {
    let mut connection = pool.acquire().await?;

    // now prepare the statements
    let login_token_email_statement = AddLoginToken::prepare_mysql(&mut connection).await?;
    let login_token_qry_statement = LoginTokenQry::prepare_mysql(&mut connection).await?;
    let invalidate_login_token_statement =
        InvalidateLoginToken::prepare_mysql(&mut connection).await?;
    let try_create_account_statement = TryCreateAccount::prepare_mysql(&mut connection).await?;
    let login_qry_statement = LoginQry::prepare_mysql(&mut connection).await?;
    let create_session_statement = CreateSession::prepare_mysql(&mut connection).await?;
    let auth_attempt_statement = AuthAttempt::prepare_mysql(&mut connection).await?;

    Ok(MySqlConnectionShared {
        login_token_email_statement,
        login_token_qry_statement,
        invalidate_login_token_statement,
        try_create_account_statement,
        login_qry_statement,
        create_session_statement,
        auth_attempt_statement,
    })
}

pub(crate) fn prepare_email(details: &EmailDetails) -> anyhow::Result<EmailShared> {
    EmailShared::new(
        &details.relay,
        details.relay_port,
        &details.email_from,
        &details.username,
        &details.password,
    )
}

pub(crate) async fn prepare_http(
    details: &HttpServerDetails,
    mysql: MySqlConnectionShared,
    email: EmailShared,
    account_server: &AccountServerDetails,
    pool: &Pool<MySql>,
) -> anyhow::Result<(TcpListener, Router, Arc<Shared>)> {
    let shared = Arc::new(Shared {
        mysql,
        email,
        signing_key: SigningKey::from_slice(&account_server.signing_key)?,
    });

    // prepare socket
    let tcp_socket = TcpSocket::new_v4()?;
    tcp_socket.set_reuseaddr(true)?;
    tcp_socket.bind(format!("127.0.0.1:{}", details.port).parse()?)?;

    let listener = tcp_socket.listen(1024).unwrap();

    // build http server
    let mut app = axum::Router::new();

    let shared_clone = shared.clone();
    let pool_clone = pool.clone();
    app = app.route(
        "/login-token-email",
        axum::routing::post(move |payload: Json<_>| {
            login_token_email(shared_clone, pool_clone, payload)
        }),
    );
    let shared_clone = shared.clone();
    let pool_clone = pool.clone();
    app = app.route(
        "/login",
        axum::routing::post(move |payload: Json<_>| {
            login_request(shared_clone, pool_clone, payload)
        }),
    );
    let shared_clone = shared.clone();
    let pool_clone = pool.clone();
    app = app.route(
        "/sign",
        axum::routing::post(move |payload: Json<_>| {
            sign_request(shared_clone, pool_clone, payload)
        }),
    );
    /*let shared_clone = shared.clone();
    let pool_clone = pool.clone();
    app = app.route(
        "/login",
        axum::routing::get(move |qry: extract::Query<_>| {
            complete_register(shared_clone, pool_clone, qry)
        }),
    );
    let shared_clone = shared.clone();
    let pool_clone = pool.clone();
    app = app.route(
        "/sign",
        axum::routing::get(move |qry: extract::Query<_>| {
            admin_account_verify_game_server_group(shared_clone, pool_clone, qry)
        }),
    );
    let shared_clone = shared.clone();
    let pool_clone = pool.clone();
    app = app.route(
        "/logout",
        axum::routing::post(move |payload: Json<_>| {
            create_session_request(shared_clone, pool_clone, payload)
        }),
    );
    let shared_clone = shared.clone();
    let pool_clone = pool.clone();
    app = app.route(
        "/change-email",
        axum::routing::post(move |payload: Json<_>| {
            auth_request(shared_clone, pool_clone, payload)
        }),
    );
    let shared_clone = shared.clone();
    let pool_clone = pool.clone();
    app = app.route(
        "/add-email",
        axum::routing::post(move |payload: Json<_>| async move {
            otp_request(shared_clone, pool_clone, payload)
        }),
    );
    let shared_clone = shared.clone();
    let pool_clone = pool.clone();
    app = app.route(
        "/add-steam",
        axum::routing::post(move |payload: Json<_>| {
            register_token_request(shared_clone, pool_clone, payload)
        }),
    );*/

    Ok((listener, app, shared))
}

pub(crate) async fn prepare(
    details: &Details,
) -> anyhow::Result<(TcpListener, Router, Pool<MySql>)> {
    // first connect to the database
    let pool = prepare_db(&details.db).await?;

    let mysql = prepare_mysql(&pool).await?;
    let email = prepare_email(&details.email)?;
    let (listener, app, _) =
        prepare_http(&details.http, mysql, email, &details.account_server, &pool).await?;

    Ok((listener, app, pool))
}

pub(crate) async fn run(listener: TcpListener, app: Router) -> anyhow::Result<()> {
    axum::serve(listener, app).await?;
    Ok(())
}

#[tokio::main]
async fn main() {
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "info");
    }
    env_logger::init();

    let mut cmd = command!()
        .about("The account server using http & mysql.")
        .arg(
            Arg::new("setup")
                .long("setup")
                .help("Setup the account server, e.g. fill the mysql tables.")
                .required(false)
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("cleanup")
                .long("cleanup")
                .help("Cleanup the account server, e.g. remove the mysql tables.")
                .required(false)
                .action(ArgAction::SetTrue),
        );
    cmd.build();
    let m = cmd.get_matches();

    let print_settings_err = || {
        log::error!(
            "a settings.json looks like this\n{}",
            serde_json::to_string_pretty(&Details {
                db: DbDetails {
                    host: "localhost".to_string(),
                    port: 3306,
                    database: "ddnet_accounts".to_string(),
                    username: "user".to_string(),
                    password: "password".to_string(),
                },
                http: HttpServerDetails { port: 443 },
                email: EmailDetails {
                    relay: "emails.localhost".to_string(),
                    relay_port: 465,
                    username: "account".to_string(),
                    password: "email-password".to_string(),
                    email_from: "account@localhost".to_string(),
                },
                account_server: AccountServerDetails {
                    signing_key: vec![0, 0, 0, 0]
                }
            })
            .unwrap()
        )
    };

    let Ok(cfg) = tokio::fs::read("settings.json").await else {
        log::error!("no settings.json found, please create one.");
        print_settings_err();

        panic!("failed to find settings.json, see log for more information");
    };

    let Ok(details) = serde_json::from_slice::<Details>(&cfg) else {
        log::error!("settings.json was invalid.");
        print_settings_err();

        panic!("settings were not a valid json file, see log for more information");
    };

    if m.value_source("setup")
        .is_some_and(|s| matches!(s, ValueSource::CommandLine))
    {
        let pool = prepare_db(&details.db).await.unwrap();
        setup_mysql(&pool).await.unwrap();
    } else if m
        .value_source("cleanup")
        .is_some_and(|s| matches!(s, ValueSource::CommandLine))
    {
        let pool = prepare_db(&details.db).await.unwrap();
        delete_mysql(&pool).await.unwrap();
    } else {
        let (listener, app, _) = prepare(&details).await.unwrap();
        run(listener, app).await.unwrap();
    }
}
