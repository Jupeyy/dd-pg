use std::{
    fmt::Debug, future::Future, ops::Deref, path::PathBuf, pin::Pin, str::FromStr, sync::Arc,
};

use accounts_base::{
    cert::generate_self_signed, cert::CertifiedKey, client::account_data::AccountDataForClient,
};
use async_trait::async_trait;
use base_io::{io::Io, io_batcher::IoBatcherTask};
use client_http_fs::{
    client::ClientHttpTokioFs,
    fs::Fs,
    profiles::{Certificate, Profiles, ProfilesLoading},
};
use client_ui::main_menu::profiles_interface::ProfilesInterface;
use url::Url;

use crate::account::AccountHttp;

pub type FactoryTy = Arc<
    dyn Fn(
            PathBuf,
        )
            -> Pin<Box<dyn Future<Output = anyhow::Result<ClientHttpTokioFs>> + Sync + Send>>
        + Sync
        + Send,
>;

pub struct Factory(FactoryTy);
impl Debug for Factory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("F").finish()
    }
}

impl Deref for Factory {
    type Target = dyn Fn(
        PathBuf,
    ) -> Pin<
        Box<dyn Future<Output = anyhow::Result<ClientHttpTokioFs>> + Sync + Send>,
    >;

    fn deref(&self) -> &Self::Target {
        self.0.as_ref()
    }
}

pub struct Accounts(Profiles<ClientHttpTokioFs, Factory>);

impl Accounts {
    pub fn new(loading: AccountsLoading) -> Self {
        let loading = loading.task.get_storage().unwrap();

        Self(Profiles::new(loading))
    }

    /// Tries to refresh the account certificate by the account server.
    /// This is only done if the cert is about to expire.
    pub async fn try_refresh_account_cert(&self) -> anyhow::Result<()> {
        self.0.signed_cert_and_key_pair().await;
        Ok(())
    }

    pub async fn connect_to_game_server(
        &self,
    ) -> (AccountDataForClient, Certificate, Option<anyhow::Error>) {
        self.0.signed_cert_and_key_pair().await
    }
}

#[async_trait]
impl ProfilesInterface for Accounts {
    async fn login_email_token(&self, email: email_address::EmailAddress) -> anyhow::Result<()> {
        Ok(())
    }

    fn profiles(&self) -> (Vec<String>, String) {
        self.0.profiles()
    }
}

pub struct AccountsLoading {
    task: IoBatcherTask<ProfilesLoading<ClientHttpTokioFs, Factory>>,
}

impl AccountsLoading {
    pub fn new(io: &Io) -> Self {
        let secure_base_path = io.fs.get_secure_path();
        let http = io.http.clone();
        let factory = Arc::new(Factory(Arc::new(move |path| {
            let http = http.clone();
            Box::pin(async move {
                Ok(ClientHttpTokioFs {
                    http: Arc::new(AccountHttp {
                        base_url: Url::from_str("https://pg.ddnet.org:54321/").unwrap(),
                        http,
                    }),
                    fs: Fs::new(path).await?,
                })
            })
        })));
        Self {
            task: io
                .io_batcher
                .spawn(async move { ProfilesLoading::new(secure_base_path, factory).await }),
        }
    }
}
