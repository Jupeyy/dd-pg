use std::{num::NonZeroU32, sync::Arc, time::Duration};

use lettre::SmtpTransport;
use parking_lot::Mutex;
use sqlx::{Any, Pool};
use tokio::task::JoinHandle;

use crate::{
    email::{EmailHook, EmailShared},
    prepare_db, prepare_http, prepare_statements, run, setup,
    shared::Shared,
};

pub async fn test_setup() -> anyhow::Result<Pool<Any>> {
    prepare_db(&crate::DbDetails {
        host: "localhost".into(),
        port: 3306,
        database: "ddnet_account_test".into(),
        username: "ddnet-account-test".into(),
        password: "test".into(),
        ca_cert_path: "/etc/mysql/ssl/ca-cert.pem".into(),
    })
    .await
}

pub struct TestAccServer {
    pub(crate) server: JoinHandle<anyhow::Result<()>>,
    pub(crate) pool: Pool<Any>,
    pub(crate) shared: Arc<Shared>,
}

impl TestAccServer {
    pub(crate) async fn new(
        token: Arc<Mutex<String>>,
        account_token: Arc<Mutex<String>>,
        limit: bool,
    ) -> anyhow::Result<Self> {
        let pool = test_setup().await?;

        if let Err(err) = setup::delete(&pool).await {
            println!("warning: {}", err);
        }
        setup::setup(&pool).await?;

        let db = prepare_statements(&pool).await?;
        let mut email: EmailShared =
            ("test@localhost", SmtpTransport::unencrypted_localhost()).into();
        #[derive(Debug)]
        struct EmailReader {
            token: Arc<Mutex<String>>,
            account_token: Arc<Mutex<String>>,
        }
        impl EmailHook for EmailReader {
            fn on_mail(&self, email_subject: &str, email_body: &str) {
                let subject = email_subject.to_lowercase();
                if subject.contains("login") {
                    let reg = regex::Regex::new(r".*```\n(.*)\n```.*").unwrap();
                    let (_, [token]): (&str, [&str; 1]) =
                        reg.captures_iter(email_body).next().unwrap().extract();
                    dbg!(token);
                    *self.token.lock() = token.to_string();
                } else if subject.contains("account token") {
                    let reg = regex::Regex::new(r".*```\n(.*)\n```.*").unwrap();
                    let (_, [account_token]): (&str, [&str; 1]) =
                        reg.captures_iter(email_body).next().unwrap().extract();
                    dbg!(account_token);
                    *self.account_token.lock() = account_token.to_string();
                }
            }
        }
        email.set_hook(EmailReader {
            token: token.clone(),
            account_token: account_token.clone(),
        });
        let limit = if limit {
            crate::LimiterSettings::default()
        } else {
            crate::LimiterSettings {
                login_tokens: crate::LimiterValues {
                    time_until_another_attempt: Duration::from_nanos(1),
                    initial_request_count: NonZeroU32::new(u32::MAX).unwrap(),
                },
                login: crate::LimiterValues {
                    time_until_another_attempt: Duration::from_nanos(1),
                    initial_request_count: NonZeroU32::new(u32::MAX).unwrap(),
                },
            }
        };
        let (listener, app, shared) = prepare_http(
            &crate::HttpServerDetails { port: 4433 },
            db,
            email,
            &pool,
            &limit,
        )
        .await?;

        let pool_clone = pool.clone();
        let shared_clone = shared.clone();
        let server =
            tokio::spawn(async move { run(listener, app, pool_clone, shared_clone).await });

        Ok(Self {
            server,
            pool,
            shared,
        })
    }

    pub(crate) async fn destroy(self) -> anyhow::Result<()> {
        self.server.abort();

        let _ = self.server.await;

        setup::delete(&self.pool).await?;
        anyhow::Ok(())
    }
}

pub struct TestGameServer {
    pool: Pool<Any>,
    pub(crate) game_server_data: Arc<account_game_server::shared::Shared>,
}

impl TestGameServer {
    pub(crate) async fn new(pool: &Pool<Any>) -> anyhow::Result<Self> {
        // make sure the tables are gone
        let _ = account_game_server::setup::delete(pool).await;
        account_game_server::setup::setup(pool).await?;

        let game_server_data = account_game_server::prepare::prepare(pool).await?;

        Ok(Self {
            pool: pool.clone(),
            game_server_data,
        })
    }

    pub(crate) async fn destroy(self) -> anyhow::Result<()> {
        account_game_server::setup::delete(&self.pool).await?;
        Ok(())
    }
}
