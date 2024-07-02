use std::{
    collections::HashMap,
    fmt::Debug,
    future::Future,
    ops::Deref,
    path::{Path, PathBuf},
    pin::Pin,
    sync::Arc,
};

use account_client::{errors::HttpLikeError, interface::Io, sign::SignResult};
use accounts_base::{
    cert::generate_self_signed,
    client::account_data::{key_pair, AccountDataForClient},
};
use anyhow::anyhow;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use x509_cert::der::Decode;

pub use x509_cert::Certificate;

use crate::fs::Fs;

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
struct ProfilesState {
    pub profiles: Vec<String>,
    pub cur_profile: String,
}

impl ProfilesState {
    async fn load_or_default(path: &Path) -> Self {
        if let Ok(file) = tokio::fs::read(path.join("profiles.json"))
            .await
            .map_err(|err| anyhow!(err))
            .and_then(|file| serde_json::from_slice(&file).map_err(|err| anyhow!(err)))
        {
            file
        } else {
            Default::default()
        }
    }

    async fn save(&self, path: &Path) -> anyhow::Result<()> {
        let tmp_file = tempfile::Builder::new().make_in(path, |_| Ok(()))?;
        let file_path = tmp_file.path().to_path_buf();
        let tmp_path = tmp_file.into_temp_path();
        tokio::fs::write(file_path, serde_json::to_string_pretty(self)?.as_bytes()).await?;
        tokio::fs::rename(tmp_path.keep()?, path.join("profiles.json")).await?;
        Ok(())
    }
}

#[derive(Debug, Default, Clone)]
pub enum ProfileCert {
    #[default]
    None,
    Fetching,
    CertAndKeys {
        cert: Certificate,
        key_pair: AccountDataForClient,
    },
}

#[derive(Debug)]
pub struct ActiveProfile<C: Io + Debug> {
    client: Arc<C>,
    cur_cert: Arc<Mutex<ProfileCert>>,
}

#[derive(Debug, Default)]
pub struct ActiveProfiles<C: Io + Debug> {
    profiles: HashMap<String, ActiveProfile<C>>,
    cur_profile: String,
}

/// Helper for multiple account profiles.
#[derive(Debug)]
pub struct Profiles<
    C: Io + Debug,
    F: Deref<
            Target = dyn Fn(
                PathBuf,
            )
                -> Pin<Box<dyn Future<Output = anyhow::Result<C>> + Sync + Send>>,
        > + Debug
        + Sync
        + Send,
> {
    profiles: parking_lot::Mutex<ActiveProfiles<C>>,
    factory: Arc<F>,
    secure_base_path: PathBuf,
    cur_warnings: parking_lot::Mutex<Vec<String>>,
    fs: Fs,
}

impl<
        C: Io + Debug,
        F: Deref<
                Target = dyn Fn(
                    PathBuf,
                )
                    -> Pin<Box<dyn Future<Output = anyhow::Result<C>> + Sync + Send>>,
            > + Debug
            + Sync
            + Send,
    > Profiles<C, F>
{
    fn to_profile_states(profiles: &ActiveProfiles<C>) -> ProfilesState {
        let mut res = ProfilesState::default();

        res.profiles.extend(profiles.profiles.keys().cloned());
        res.cur_profile = profiles.cur_profile.clone();

        res
    }

    fn email_to_path_friendy(email: &email_address::EmailAddress) -> String {
        email.as_str().replace('@', "_at_").replace('.', "_dot_")
    }

    pub fn new(loading: ProfilesLoading<C, F>) -> Self {
        Self {
            profiles: loading.profiles,
            factory: loading.factory,
            secure_base_path: loading.secure_base_path,
            cur_warnings: Default::default(),
            fs: loading.fs,
        }
    }

    /// logs in a new user and adds it to the profiles
    pub async fn login_email_token(
        &self,
        email: email_address::EmailAddress,
    ) -> anyhow::Result<()> {
        let profile_name = Self::email_to_path_friendy(&email);
        let path = self.secure_base_path.join(&profile_name);
        let account_client = Arc::new((self.factory)(path).await?);

        account_client::login_token_email::login_token_email(email, account_client.as_ref())
            .await?;

        Ok(())
    }

    /// try to login via login token previously create with e.g. [`Self::login_email_token`]
    pub async fn login(
        &self,
        email: email_address::EmailAddress,
        login_token_b64: String,
    ) -> anyhow::Result<()> {
        let profile_name = Self::email_to_path_friendy(&email);
        let path = self.secure_base_path.join(&profile_name);
        let account_client = Arc::new((self.factory)(path).await?);
        let _ = account_client::login::login(login_token_b64, account_client.as_ref()).await?;

        let profile = ActiveProfile {
            client: account_client,
            cur_cert: Default::default(),
        };

        let profiles_state;
        {
            let mut profiles = self.profiles.lock();
            profiles.profiles.insert(profile_name.clone(), profile);
            profiles.cur_profile = profile_name;
            profiles_state = Self::to_profile_states(&profiles);
            drop(profiles);
        }

        profiles_state.save(&self.secure_base_path).await?;

        self.signed_cert_and_key_pair().await;

        Ok(())
    }

    /// removes the profile
    async fn logout_impl(&self, profile_name: &str) -> anyhow::Result<()> {
        let profiles_state;
        {
            let mut profiles = self.profiles.lock();
            profiles.profiles.remove(profile_name);
            if profiles.cur_profile == profile_name {
                profiles.cur_profile = "".into();
            }
            profiles_state = Self::to_profile_states(&profiles);
            drop(profiles);
        }

        profiles_state.save(&self.secure_base_path).await?;

        Ok(())
    }

    /// Gets a _recently_ signed cerificate from the accounts server
    /// and the key pair of the client.
    /// If an error occurred a self signed cert & key-pair will still be generated to
    /// allow playing at all cost.
    /// It's up to the implementation how it wants to inform the user about
    /// this error.
    pub async fn signed_cert_and_key_pair(
        &self,
    ) -> (AccountDataForClient, Certificate, Option<anyhow::Error>) {
        let mut cur_cert_der = None;
        let mut account_client = None;
        let mut cur_profile = None;
        {
            let profiles = self.profiles.lock();
            if let Some(profile) = profiles.profiles.get(&profiles.cur_profile) {
                cur_cert_der = Some(profile.cur_cert.clone());
                account_client = Some(profile.client.clone());
                cur_profile = Some(profiles.cur_profile.clone());
            }
            drop(profiles);
        }

        fn gen_keys(
            err: anyhow::Error,
        ) -> (AccountDataForClient, Certificate, Option<anyhow::Error>) {
            let (private_key, public_key) = key_pair();

            let cert = generate_self_signed(&private_key).unwrap();
            (
                AccountDataForClient {
                    private_key,
                    public_key,
                },
                cert,
                Some(err),
            )
        }

        if let Some(((cur_cert, client), cur_profile)) =
            cur_cert_der.zip(account_client).zip(cur_profile)
        {
            let try_fetch;
            {
                let mut cert = cur_cert.lock();
                try_fetch = matches!(*cert, ProfileCert::None);
                if try_fetch {
                    *cert = ProfileCert::Fetching;
                }
            }

            if try_fetch {
                match account_client::sign::sign(client.as_ref()).await {
                    Ok(sign_data) => {
                        if let Ok(cert) = Certificate::from_der(&sign_data.certificate_der) {
                            *cur_cert.lock() = ProfileCert::CertAndKeys {
                                cert: cert.clone(),
                                key_pair: sign_data.session_key_pair.clone(),
                            };
                            (sign_data.session_key_pair, cert, None)
                        } else {
                            gen_keys(anyhow!("account server did not return a valid certificate, please contact a developer."))
                        }
                    }
                    Err(err) => {
                        *cur_cert.lock() = ProfileCert::None;
                        // if the error was a file system error
                        // or session was invalid for other reasons, then remove that profile.
                        if matches!(
                            err,
                            SignResult::FsLikeError(_)
                                | SignResult::SessionWasInvalid
                                | SignResult::HttpLikeError(HttpLikeError::Other(_))
                        ) {
                            // try to remove that profile
                            let _ = self.logout_impl(&cur_profile).await;
                        }
                        gen_keys(err.into())
                    }
                }
            } else {
                let ProfileCert::CertAndKeys { cert, key_pair } = cur_cert.lock().clone() else {
                    return gen_keys(anyhow!("no cert or key found."));
                };

                (key_pair, cert, None)
            }
        } else {
            gen_keys(anyhow!("No current profile found."))
        }
    }

    /// Currently loaded profiles
    pub fn profiles(&self) -> (Vec<String>, String) {
        let profiles = self.profiles.lock();
        let profiles = Self::to_profile_states(&profiles);
        (profiles.profiles, profiles.cur_profile)
    }
}

#[derive(Debug)]
pub struct ProfilesLoading<
    C: Io + Debug,
    F: Deref<
            Target = dyn Fn(
                PathBuf,
            )
                -> Pin<Box<dyn Future<Output = anyhow::Result<C>> + Sync + Send>>,
        > + Debug
        + Sync
        + Send,
> {
    pub profiles: parking_lot::Mutex<ActiveProfiles<C>>,
    pub factory: Arc<F>,
    pub secure_base_path: PathBuf,
    fs: Fs,
}

impl<
        C: Io + Debug,
        F: Deref<
                Target = dyn Fn(
                    PathBuf,
                )
                    -> Pin<Box<dyn Future<Output = anyhow::Result<C>> + Sync + Send>>,
            > + Debug
            + Sync
            + Send,
    > ProfilesLoading<C, F>
{
    pub async fn new(secure_base_path: PathBuf, factory: Arc<F>) -> anyhow::Result<Self> {
        let profiles_state = ProfilesState::load_or_default(&secure_base_path).await;
        let mut profiles: HashMap<String, ActiveProfile<C>> = Default::default();
        for profile in profiles_state.profiles {
            profiles.insert(
                profile.clone(),
                ActiveProfile {
                    client: Arc::new(factory(secure_base_path.join(profile)).await?),
                    cur_cert: Default::default(),
                },
            );
        }
        Ok(Self {
            profiles: parking_lot::Mutex::new(ActiveProfiles {
                profiles,
                cur_profile: profiles_state.cur_profile,
            }),
            factory,
            fs: Fs::new(secure_base_path.clone()).await?,
            secure_base_path,
        })
    }
}
