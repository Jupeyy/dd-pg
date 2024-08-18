//! This is the http + db implementation for the account server.

#![deny(missing_docs)]
#![deny(warnings)]
#![deny(clippy::nursery)]
#![deny(clippy::all)]

pub(crate) mod account_token;
mod certs;
pub(crate) mod db;
pub(crate) mod delete;
pub(crate) mod email;
pub(crate) mod internal_err;
pub(crate) mod login;
pub(crate) mod login_token_email;
mod logout;
pub(crate) mod setup;
pub(crate) mod shared;
pub(crate) mod sign;
pub(crate) mod update;

#[cfg(test)]
mod tests;

use account_sql::query::Query;
use account_token::{
    account_token_email,
    queries::{AccountTokenQry, AddAccountTokenEmail, InvalidateAccountToken},
};
use anyhow::anyhow;
use axum::{Json, Router};
use certs::{
    certs_request, generate_key_and_cert, get_certs,
    queries::{AddCert, GetCerts},
    store_cert, PrivateKeys,
};
use clap::{command, parser::ValueSource, Arg, ArgAction};
use db::DbConnectionShared;
use delete::{
    delete_request, delete_sessions_request,
    queries::{RemoveAccount, RemoveSessions},
};
use email::EmailShared;
use login::{
    login_request,
    queries::{CreateSession, InvalidateLoginToken, LoginQry, LoginTokenQry, TryCreateAccount},
};
use login_token_email::{login_token_email, queries::AddLoginToken};
use logout::{logout_request, queries::RemoveSession};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use shared::Shared;
use sign::{queries::AuthAttempt, sign_request};
use sqlx::{any::AnyPoolOptions, mysql::MySqlConnectOptions, Any, AnyPool, Pool};
use std::{
    path::PathBuf,
    sync::Arc,
    time::{Duration, SystemTime},
};
use tokio::net::{TcpListener, TcpSocket};
use update::{
    queries::{CleanupAccountTokens, CleanupCerts, CleanupLoginTokens},
    update,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DbDetails {
    host: String,
    port: u16,
    database: String,
    username: String,
    password: String,
    ca_cert_path: PathBuf,
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
struct Details {
    db: DbDetails,
    http: HttpServerDetails,
    email: EmailDetails,
}

pub(crate) async fn prepare_db(details: &DbDetails) -> anyhow::Result<Pool<Any>> {
    Ok(AnyPoolOptions::new()
        .max_connections(200)
        .connect_with(
            MySqlConnectOptions::new()
                .charset("utf8mb4")
                .host(&details.host)
                .port(details.port)
                .database(&details.database)
                .username(&details.username)
                .password(&details.password)
                .ssl_mode(sqlx::mysql::MySqlSslMode::Required)
                .ssl_ca(&details.ca_cert_path)
                .into(),
        )
        .await?)
}

pub(crate) async fn prepare_statements(pool: &Pool<Any>) -> anyhow::Result<DbConnectionShared> {
    let mut connection = pool.acquire().await?;

    // now prepare the statements
    let login_token_email_statement = AddLoginToken::prepare(&mut connection).await?;
    let login_token_qry_statement = LoginTokenQry::prepare(&mut connection).await?;
    let invalidate_login_token_statement = InvalidateLoginToken::prepare(&mut connection).await?;
    let try_create_account_statement = TryCreateAccount::prepare(&mut connection).await?;
    let login_qry_statement = LoginQry::prepare(&mut connection).await?;
    let create_session_statement = CreateSession::prepare(&mut connection).await?;
    let logout_statement = RemoveSession::prepare(&mut connection).await?;
    let auth_attempt_statement = AuthAttempt::prepare(&mut connection).await?;
    let account_token_email_statement = AddAccountTokenEmail::prepare(&mut connection).await?;
    let account_token_qry_statement = AccountTokenQry::prepare(&mut connection).await?;
    let invalidate_account_token_statement =
        InvalidateAccountToken::prepare(&mut connection).await?;
    let remove_sessions_statement = RemoveSessions::prepare(&mut connection).await?;
    let remove_account_statement = RemoveAccount::prepare(&mut connection).await?;
    let add_cert_statement = AddCert::prepare(&mut connection).await?;
    let get_certs_statement = GetCerts::prepare(&mut connection).await?;
    let cleanup_login_tokens_statement = CleanupLoginTokens::prepare(&mut connection).await?;
    let cleanup_account_tokens_statement = CleanupAccountTokens::prepare(&mut connection).await?;
    let cleanup_certs_statement = CleanupCerts::prepare(&mut connection).await?;

    Ok(DbConnectionShared {
        login_token_email_statement,
        login_token_qry_statement,
        invalidate_login_token_statement,
        try_create_account_statement,
        login_qry_statement,
        create_session_statement,
        logout_statement,
        auth_attempt_statement,
        account_token_email_statement,
        account_token_qry_statement,
        invalidate_account_token_statement,
        remove_sessions_statement,
        remove_account_statement,
        add_cert_statement,
        get_certs_statement,
        cleanup_login_tokens_statement,
        cleanup_account_tokens_statement,
        cleanup_certs_statement,
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
    db: DbConnectionShared,
    email: EmailShared,
    pool: &Pool<Any>,
) -> anyhow::Result<(TcpListener, Router, Arc<Shared>)> {
    let keys = tokio::fs::read("signing_keys.json")
        .await
        .map_err(|err| anyhow!(err))
        .and_then(|key| serde_json::from_slice::<PrivateKeys>(&key).map_err(|err| anyhow!(err)));

    let keys = if let Ok(keys) = keys {
        keys
    } else {
        let (key1, cert1) = generate_key_and_cert(true)?;
        store_cert(&db, pool, &cert1).await?;

        let (key2, cert2) = generate_key_and_cert(false)?;
        store_cert(&db, pool, &cert2).await?;

        let res = PrivateKeys {
            current_key: key1,
            current_cert: cert1,
            next_key: key2,
            next_cert: cert2,
        };

        tokio::fs::write("signing_keys.json", serde_json::to_vec(&res)?).await?;

        res
    };

    let certs = get_certs(&db, pool).await?;

    let shared = Arc::new(Shared {
        db,
        email,
        signing_keys: Arc::new(RwLock::new(Arc::new(keys))),
        cert_chain: Arc::new(RwLock::new(Arc::new(certs))),
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
    let shared_clone = shared.clone();
    let pool_clone = pool.clone();
    app = app.route(
        "/account-token",
        axum::routing::post(move |qry: Json<_>| account_token_email(shared_clone, pool_clone, qry)),
    );
    let shared_clone = shared.clone();
    let pool_clone = pool.clone();
    app = app.route(
        "/delete-account",
        axum::routing::post(move |qry: Json<_>| delete_request(shared_clone, pool_clone, qry)),
    );
    let shared_clone = shared.clone();
    let pool_clone = pool.clone();
    app = app.route(
        "/delete-sessions",
        axum::routing::post(move |qry: Json<_>| {
            delete_sessions_request(shared_clone, pool_clone, qry)
        }),
    );
    let shared_clone = shared.clone();
    let pool_clone = pool.clone();
    app = app.route(
        "/logout",
        axum::routing::post(move |qry: Json<_>| logout_request(shared_clone, pool_clone, qry)),
    );
    let shared_clone = shared.clone();
    app = app.route(
        "/certs",
        axum::routing::get(move || certs_request(shared_clone)),
    );

    Ok((listener, app, shared))
}

pub(crate) async fn prepare(
    details: &Details,
) -> anyhow::Result<(TcpListener, Router, Pool<Any>, Arc<Shared>)> {
    // first connect to the database
    let pool = prepare_db(&details.db).await?;

    let db = prepare_statements(&pool).await?;
    let email = prepare_email(&details.email)?;
    let (listener, app, shared) = prepare_http(&details.http, db, email, &pool).await?;

    Ok((listener, app, pool, shared))
}

pub(crate) async fn generate_new_signing_keys(pool: &AnyPool, shared: &Arc<Shared>) -> u64 {
    // once per day check if a new signing key should be created
    let mut next_sleep_time = 60 * 60 * 24;

    let check_keys = shared.signing_keys.read().clone();
    if SystemTime::now() + Duration::from_secs(60 * 60 * 24 * 7)
        >= check_keys
            .current_cert
            .tbs_certificate
            .validity
            .not_after
            .to_system_time()
    {
        // create a new key & cert, switch next key to current
        if let Ok((key, cert)) = generate_key_and_cert(false) {
            let store_res = store_cert(&shared.db, pool, &cert).await;
            if store_res.is_err() {
                next_sleep_time = 60 * 60 * 2;
            } else if let Ok(certs) = get_certs(&shared.db, pool).await {
                let cur_keys = shared.signing_keys.read().clone();
                let new_keys = Arc::new(PrivateKeys {
                    current_key: cur_keys.next_key.clone(),
                    current_cert: cur_keys.next_cert.clone(),
                    next_key: key,
                    next_cert: cert,
                });
                if let Ok(val) = serde_json::to_vec(new_keys.as_ref()) {
                    if tokio::fs::write("signing_keys.json", val).await.is_ok() {
                        *shared.cert_chain.write() = Arc::new(certs);
                        *shared.signing_keys.write() = new_keys;
                    } else {
                        next_sleep_time = 60 * 60 * 2;
                    }
                } else {
                    next_sleep_time = 60 * 60 * 2;
                }
            } else {
                next_sleep_time = 60 * 60 * 2;
            }
        } else {
            next_sleep_time = 60 * 60 * 2;
        }
    }
    next_sleep_time
}

async fn regenerate_signing_keys_and_certs(pool: AnyPool, shared: Arc<Shared>) -> ! {
    loop {
        let next_sleep_time = generate_new_signing_keys(&pool, &shared).await;

        tokio::time::sleep(Duration::from_secs(next_sleep_time)).await;

        // get latest certs
        if let Ok(certs) = get_certs(&shared.db, &pool).await {
            *shared.cert_chain.write() = Arc::new(certs);
        }
    }
}

// https://github.com/tokio-rs/tokio/issues/5616
#[allow(clippy::redundant_pub_crate)]
pub(crate) async fn run(
    listener: TcpListener,
    app: Router,
    pool: AnyPool,
    shared: Arc<Shared>,
) -> anyhow::Result<()> {
    let pool_clone = pool.clone();
    let shared_clone = shared.clone();
    tokio::select!(
        err = async move { axum::serve(listener, app).await } => {
           err?;
        },
        _ = async move {
            regenerate_signing_keys_and_certs(pool, shared).await
        } => {}
        _ = async move {
            update(pool_clone, shared_clone).await
        } => {}
    );
    Ok(())
}

#[tokio::main]
async fn main() {
    if std::env::var("RUST_LOG").is_err() {
        // rust nightly compatibility
        #[allow(unused_unsafe)]
        unsafe {
            std::env::set_var("RUST_LOG", "info")
        };
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
                    ca_cert_path: "/etc/mysql/ssl/ca-cert.pem".into()
                },
                http: HttpServerDetails { port: 443 },
                email: EmailDetails {
                    relay: "emails.localhost".to_string(),
                    relay_port: 465,
                    username: "account".to_string(),
                    password: "email-password".to_string(),
                    email_from: "account@localhost".to_string(),
                },
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
        setup::setup(&pool).await.unwrap();
    } else if m
        .value_source("cleanup")
        .is_some_and(|s| matches!(s, ValueSource::CommandLine))
    {
        let pool = prepare_db(&details.db).await.unwrap();
        setup::delete(&pool).await.unwrap();
    } else {
        let (listener, app, pool, shared) = prepare(&details).await.unwrap();
        run(listener, app, pool, shared).await.unwrap();
    }
}
