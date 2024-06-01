//! This is the http + mysql implementation for the account server.

#![deny(missing_docs)]
#![deny(warnings)]
#![deny(clippy::nursery)]
#![deny(clippy::all)]

pub(crate) mod auth;
pub(crate) mod complete_register;
pub(crate) mod email;
pub(crate) mod internal_err;
pub(crate) mod key_store;
pub(crate) mod mysql;
pub(crate) mod otp;
pub(crate) mod otps;
pub(crate) mod password_forgot;
pub(crate) mod password_reset;
pub(crate) mod register;
pub(crate) mod register_token;
pub(crate) mod register_tokens;
pub(crate) mod session;
pub(crate) mod setup;
pub(crate) mod shared;
pub(crate) mod verify_game_server_group;

#[cfg(test)]
mod tests;

use std::{str::FromStr, sync::Arc};

use account_sql::query::Query;
use auth::{auth_request, queries::AuthAttempt};
use axum::{extract, Json, Router};
use clap::{command, parser::ValueSource, Arg, ArgAction};
use complete_register::{complete_register, queries::VerifyAccount};
use email::EmailShared;
use key_store::{
    game_server_queries,
    queries::{GetGameServerGroupKeyPair, StoreGameServerGroupKeyPair},
    server_group_key_pair_request, store_server_group_key_pair_request,
};
use mysql::MySqlConnectionShared;
use otp::otp_request;
use otps::Otps;
use password_forgot::{
    password_forgot_request,
    queries::{AddResetCode, EmailExistsCheck},
};
use password_reset::{
    password_reset_request,
    queries::VerifyResetCodeAndResetAccount,
    rem_queries::{GetAccountIdResetCode, RemClientKeys, RemGameServerKeys, RemSessions},
};
use register::{
    queries::{AddAccount, AddVerifyToken},
    register,
};
use register_token::{account_id_from_register_token_request, register_token_request};
use register_tokens::RegisterTokens;
use serde::{Deserialize, Serialize};
use session::{
    create_session_request,
    queries::{CreateSession, LoginAttempt},
};
use setup::{delete_mysql, setup_mysql};
use shared::Shared;
use sqlx::{
    mysql::{MySqlConnectOptions, MySqlPoolOptions},
    MySql, Pool,
};
use tokio::net::{TcpListener, TcpSocket};
use url::Url;
use verify_game_server_group::{
    admin_account_verify_game_server_group, queries::VerifyAccountGameServerGroup,
};

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
    url: Url,
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
struct AdminDetails {
    /// Admin password that can be used to e.g.
    /// verify accounts as game server group.
    admin_password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Details {
    db: DbDetails,
    http: HttpServerDetails,
    email: EmailDetails,
    admin: AdminDetails,
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
    let register_statement = AddAccount::prepare_mysql(&mut connection).await?;
    let complete_register_statement = VerifyAccount::prepare_mysql(&mut connection).await?;
    let admin_verify_account_game_server_group_statement =
        VerifyAccountGameServerGroup::prepare_mysql(&mut connection).await?;
    let add_verify_token = AddVerifyToken::prepare_mysql(&mut connection).await?;
    let login_attempt_statement = LoginAttempt::prepare_mysql(&mut connection).await?;
    let create_session_statement = CreateSession::prepare_mysql(&mut connection).await?;
    let auth_attempt_statement = AuthAttempt::prepare_mysql(&mut connection).await?;
    let email_exists_statement = EmailExistsCheck::prepare_mysql(&mut connection).await?;
    let add_reset_code_statement = AddResetCode::prepare_mysql(&mut connection).await?;
    let verify_reset_code_and_reset_account_statement =
        VerifyResetCodeAndResetAccount::prepare_mysql(&mut connection).await?;
    let get_game_server_group_key_pair_statement =
        GetGameServerGroupKeyPair::prepare_mysql(&mut connection).await?;
    let game_server_get_game_server_group_key_pair_statement =
        game_server_queries::GetGameServerGroupKeyPair::prepare_mysql(&mut connection).await?;
    let store_game_server_group_key_pair_statement =
        StoreGameServerGroupKeyPair::prepare_mysql(&mut connection).await?;
    let game_server_store_game_server_group_key_pair_statement =
        game_server_queries::StoreGameServerGroupKeyPair::prepare_mysql(&mut connection).await?;
    let get_account_id_from_reset_code_statement =
        GetAccountIdResetCode::prepare_mysql(&mut connection).await?;
    let clear_client_keys_statement = RemClientKeys::prepare_mysql(&mut connection).await?;
    let clear_game_server_key_statement = RemGameServerKeys::prepare_mysql(&mut connection).await?;
    let clear_sessions_statement = RemSessions::prepare_mysql(&mut connection).await?;

    Ok(MySqlConnectionShared {
        register_statement,
        complete_register_statement,
        admin_verify_account_game_server_group_statement,
        add_verify_token,
        login_attempt_statement,
        create_session_statement,
        auth_attempt_statement,
        email_exists_statement,
        add_reset_code_statement,
        verify_reset_code_and_reset_account_statement,
        get_game_server_group_key_pair_statement,
        game_server_get_game_server_group_key_pair_statement,
        store_game_server_group_key_pair_statement,
        game_server_store_game_server_group_key_pair_statement,
        get_account_id_from_reset_code_statement,
        clear_client_keys_statement,
        clear_game_server_key_statement,
        clear_sessions_statement,
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
    admin: AdminDetails,
    pool: &Pool<MySql>,
) -> anyhow::Result<(TcpListener, Router, Arc<Shared>)> {
    let shared = Arc::new(Shared {
        mysql,
        otps: Otps::default(),
        register_tokens: RegisterTokens::default(),
        email,
        http_url: details.url.clone(),
        admin_password: admin.admin_password,
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
        "/register",
        axum::routing::post(move |payload: Json<_>| register(shared_clone, pool_clone, payload)),
    );
    let shared_clone = shared.clone();
    let pool_clone = pool.clone();
    app = app.route(
        "/complete-register",
        axum::routing::get(move |qry: extract::Query<_>| {
            complete_register(shared_clone, pool_clone, qry)
        }),
    );
    let shared_clone = shared.clone();
    let pool_clone = pool.clone();
    app = app.route(
        "/admin-verify-account-game-server-group",
        axum::routing::get(move |qry: extract::Query<_>| {
            admin_account_verify_game_server_group(shared_clone, pool_clone, qry)
        }),
    );
    let shared_clone = shared.clone();
    let pool_clone = pool.clone();
    app = app.route(
        "/login",
        axum::routing::post(move |payload: Json<_>| {
            create_session_request(shared_clone, pool_clone, payload)
        }),
    );
    let shared_clone = shared.clone();
    let pool_clone = pool.clone();
    app = app.route(
        "/auth",
        axum::routing::post(move |payload: Json<_>| {
            auth_request(shared_clone, pool_clone, payload)
        }),
    );
    let shared_clone = shared.clone();
    let pool_clone = pool.clone();
    app = app.route(
        "/otp",
        axum::routing::post(move |payload: Json<_>| async move {
            otp_request(shared_clone, pool_clone, payload)
        }),
    );
    let shared_clone = shared.clone();
    let pool_clone = pool.clone();
    app = app.route(
        "/register-token",
        axum::routing::post(move |payload: Json<_>| {
            register_token_request(shared_clone, pool_clone, payload)
        }),
    );
    let shared_clone = shared.clone();
    app = app.route(
        "/account-id-from-register-token",
        axum::routing::post(move |payload: Json<_>| async move {
            account_id_from_register_token_request(shared_clone, payload)
        }),
    );
    let shared_clone = shared.clone();
    let pool_clone = pool.clone();
    app = app.route(
        "/password-forgot",
        axum::routing::post(move |payload: Json<_>| {
            password_forgot_request(shared_clone, pool_clone, payload)
        }),
    );
    let shared_clone = shared.clone();
    let pool_clone = pool.clone();
    app = app.route(
        "/password-reset",
        axum::routing::post(move |payload: Json<_>| {
            password_reset_request(shared_clone, pool_clone, payload)
        }),
    );
    let shared_clone = shared.clone();
    let pool_clone = pool.clone();
    app = app.route(
        "/server-group-key-pair",
        axum::routing::post(move |payload: Json<_>| {
            server_group_key_pair_request(shared_clone, pool_clone, payload)
        }),
    );
    let shared_clone = shared.clone();
    let pool_clone = pool.clone();
    app = app.route(
        "/store-server-group-key-pair",
        axum::routing::post(move |payload: Json<_>| {
            store_server_group_key_pair_request(shared_clone, pool_clone, payload)
        }),
    );

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
        prepare_http(&details.http, mysql, email, details.admin.clone(), &pool).await?;

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
                http: HttpServerDetails {
                    port: 443,
                    url: Url::from_str("https://accounts.localhost").unwrap(),
                },
                email: EmailDetails {
                    relay: "emails.localhost".to_string(),
                    relay_port: 465,
                    username: "account".to_string(),
                    password: "email-password".to_string(),
                    email_from: "account@localhost".to_string(),
                },
                admin: AdminDetails {
                    admin_password: "SupeRSecur3AdminP@ssword!".into()
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
