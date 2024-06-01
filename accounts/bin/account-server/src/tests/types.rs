use std::{str::FromStr, sync::Arc};

use lettre::SmtpTransport;
use parking_lot::Mutex;
use sqlx::{MySql, Pool};
use tokio::task::JoinHandle;
use url::Url;

use crate::{
    email::{EmailHook, EmailShared},
    prepare_db, prepare_http, prepare_mysql, run,
    setup::{delete_mysql, setup_mysql},
    shared::Shared,
};

pub async fn test_setup() -> anyhow::Result<Pool<MySql>> {
    prepare_db(&crate::DbDetails {
        host: "localhost".into(),
        port: 3306,
        database: "ddnet_account_test".into(),
        username: "ddnet-account-test".into(),
        password: "test".into(),
    })
    .await
}

pub struct TestAccServer {
    pub(crate) server: JoinHandle<anyhow::Result<()>>,
    pub(crate) pool: Pool<MySql>,
    pub(crate) shared: Arc<Shared>,
}

impl TestAccServer {
    pub(crate) async fn new(
        token: Arc<Mutex<String>>,
        reset_code: Arc<Mutex<String>>,
    ) -> anyhow::Result<Self> {
        let pool = test_setup().await?;

        let _ = delete_mysql(&pool).await;
        setup_mysql(&pool).await?;

        let mysql = prepare_mysql(&pool).await?;
        let mut email: EmailShared =
            ("test@localhost", SmtpTransport::unencrypted_localhost()).into();
        #[derive(Debug)]
        struct EmailReader {
            token: Arc<Mutex<String>>,
            reset_code: Arc<Mutex<String>>,
        }
        impl EmailHook for EmailReader {
            fn on_mail(&self, email_subject: &str, email_body: &str) {
                let subject = email_subject.to_lowercase();
                if subject.contains("registration") {
                    let reg = regex::Regex::new(r".*href='(.*)'.*").unwrap();
                    let (_, [url]): (&str, [&str; 1]) =
                        reg.captures_iter(email_body).next().unwrap().extract();
                    dbg!(url);
                    *self.token.lock() = url.to_string();
                } else if subject.contains("reset") {
                    let reg = regex::Regex::new(r".*```(.*)```.*").unwrap();
                    let (_, [code]): (&str, [&str; 1]) =
                        reg.captures_iter(email_body).next().unwrap().extract();
                    dbg!(code);
                    *self.reset_code.lock() = code.to_string();
                }
            }
        }
        email.set_hook(EmailReader {
            token: token.clone(),
            reset_code: reset_code.clone(),
        });
        let (listener, app, shared) = prepare_http(
            &crate::HttpServerDetails {
                port: 4433,
                url: Url::from_str("http://localhost:4433/")?,
            },
            mysql,
            email,
            crate::AdminDetails {
                admin_password: "test-admin-pw".into(),
            },
            &pool,
        )
        .await?;

        let server = tokio::spawn(async move { run(listener, app).await });

        Ok(Self {
            server,
            pool,
            shared,
        })
    }

    pub(crate) async fn destroy(self) -> anyhow::Result<()> {
        self.server.abort();

        delete_mysql(&self.pool).await?;
        anyhow::Ok(())
    }
}

pub struct TestGameServer {
    pool: Pool<MySql>,
    pub(crate) game_server_data: Arc<account_game_server::shared::Shared>,
}

impl TestGameServer {
    pub(crate) async fn new(pool: &Pool<MySql>) -> anyhow::Result<Self> {
        // make sure the tables are gone
        let _ = account_game_server::setup::delete_mysql(pool).await;
        account_game_server::setup::setup_mysql(pool).await?;

        let game_server_data = account_game_server::prepare::prepare(pool).await?;

        Ok(Self {
            pool: pool.clone(),
            game_server_data,
        })
    }

    pub(crate) async fn destroy(self) -> anyhow::Result<()> {
        account_game_server::setup::delete_mysql(&self.pool).await?;
        Ok(())
    }
}
