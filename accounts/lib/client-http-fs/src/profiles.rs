use std::{
    collections::HashMap,
    fmt::Debug,
    future::Future,
    net::IpAddr,
    ops::Deref,
    path::{Path, PathBuf},
    pin::Pin,
    str::FromStr,
    sync::Arc,
};

use account_client::{
    auth::{AuthClientData, AuthResult},
    game_server_group_data::GameServerGroupData,
    interface::Io,
    login::AuthLoginResult,
};
use accounts_base::{
    client::game_server_data::{generate_client_game_server_key_pair, ClientGameServerKeyPair},
    game_server::server_id::GameServerGroupId,
};
use anyhow::anyhow;
use serde::{Deserialize, Serialize};

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

#[derive(Debug, Default)]
enum ProfileAuthState {
    #[default]
    None,
    Trying,
    Authed(AuthClientData),
    Failed,
}

#[derive(Debug)]
pub struct ActiveProfile<C: Io + Debug> {
    client: Arc<C>,
    auth_data: ProfileAuthState,
}

#[derive(Debug, Default)]
pub struct ActiveProfiles<C: Io + Debug> {
    profiles: HashMap<String, ActiveProfile<C>>,
    cur_profile: String,
}

#[derive(Debug)]
pub enum GameServerMode {
    Group(GameServerGroupId),
    Ip(IpAddr),
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
    pub async fn login(
        &self,
        email: email_address::EmailAddress,
        password: &str,
    ) -> anyhow::Result<()> {
        let profile_name = Self::email_to_path_friendy(&email);
        let path = self.secure_base_path.join(&profile_name);
        let account_client = Arc::new((self.factory)(path).await?);
        let login_data =
            account_client::login::login(email, password, account_client.as_ref()).await?;

        let mut profile = ActiveProfile {
            client: account_client,
            auth_data: Default::default(),
        };

        match login_data.auth {
            AuthLoginResult::Verified(auth) => {
                profile.auth_data = ProfileAuthState::Authed(*auth);
            }
            AuthLoginResult::NotVerified => {
                profile.auth_data = ProfileAuthState::Failed;
                self.cur_warnings
                    .lock()
                    .push("You have to verify your account.".into());
            }
        }

        let profiles_state;
        {
            let mut profiles = self.profiles.lock();
            profiles.profiles.insert(profile_name.clone(), profile);
            profiles.cur_profile = profile_name;
            profiles_state = Self::to_profile_states(&profiles);
            drop(profiles);
        }

        profiles_state.save(&self.secure_base_path).await?;

        Ok(())
    }

    // registers a new user and adds it to the profiles on success
    pub async fn register(
        &self,
        email: email_address::EmailAddress,
        password: &str,
    ) -> anyhow::Result<()> {
        let profile_name = Self::email_to_path_friendy(&email);
        let path = self.secure_base_path.join(&profile_name);
        let account_client = Arc::new((self.factory)(path).await?);
        let register_data =
            account_client::register::register(email, password, account_client.as_ref()).await?;

        let profile = ActiveProfile {
            client: account_client,
            auth_data: Default::default(),
        };

        let try_auth =
            !register_data.requires_verification && register_data.session_was_created.is_ok();

        if register_data.requires_verification {
            self.cur_warnings
                .lock()
                .push("You have to verify your account.".into());
        }
        if let Err(err) = register_data.session_was_created {
            self.cur_warnings
                .lock()
                .push(format!("You have login: {}", err));
        }

        let profiles_state;
        {
            let mut profiles = self.profiles.lock();
            profiles.profiles.insert(profile_name.clone(), profile);
            profiles.cur_profile = profile_name;
            profiles_state = Self::to_profile_states(&profiles);
            drop(profiles);
        }

        profiles_state.save(&self.secure_base_path).await?;

        if try_auth {
            self.try_auth_current_active().await?;
        }

        Ok(())
    }

    /// Auth the current active profile
    pub async fn try_auth_current_active(&self) -> anyhow::Result<()> {
        let should_auth;
        let client;
        let cur_profile;
        {
            let mut profiles = self.profiles.lock();
            cur_profile = profiles.cur_profile.clone();
            let profile = profiles
                .profiles
                .get_mut(&cur_profile)
                .ok_or_else(|| anyhow!("no current profile active."))?;

            should_auth = matches!(
                profile.auth_data,
                ProfileAuthState::None | ProfileAuthState::Failed
            );

            client = profile.client.clone();
            if should_auth {
                profile.auth_data = ProfileAuthState::Trying;
            }
            drop(profiles);
        }

        if should_auth {
            // try to do the auth
            let auth = match account_client::auth::auth(client.as_ref()).await {
                Ok(auth) => ProfileAuthState::Authed(auth),
                Err(err) => {
                    match &err {
                        account_client::auth::AuthResult::AccountNotVerified => {
                            self.cur_warnings
                                .lock()
                                .push("You have to verify your account.".into());
                        }
                        account_client::auth::AuthResult::Other(_)
                        | account_client::auth::AuthResult::FsLikeError(_)
                        | account_client::auth::AuthResult::MainSecretCryptFailed(_)
                        | account_client::auth::AuthResult::SessionWasInvalid => {
                            self.cur_warnings.lock().push("You have to login.".into());
                        }
                        account_client::auth::AuthResult::HttpLikeError(_) => {
                            // ignore, will be tried again later
                            // when getting the keys.
                        }
                    }
                    ProfileAuthState::Failed
                }
            };
            let mut profiles = self.profiles.lock();
            let profile = profiles
                .profiles
                .get_mut(&cur_profile)
                .ok_or_else(|| anyhow!("no current profile active."))?;
            profile.auth_data = auth;
        }

        Ok(())
    }

    /// Client is about to connect to a game server
    /// and needs its key pair for it.
    pub async fn connect_to_game_server(
        &self,
        game_server_mode: GameServerMode,
    ) -> anyhow::Result<ClientGameServerKeyPair> {
        match game_server_mode {
            GameServerMode::Group(game_server_group_id) => {
                let is_authed;
                let cur_profile;
                {
                    let mut profiles = self.profiles.lock();
                    cur_profile = profiles.cur_profile.clone();
                    let profile = profiles
                        .profiles
                        .get_mut(&cur_profile)
                        .ok_or_else(|| anyhow!("no current profile active."))?;

                    is_authed = matches!(profile.auth_data, ProfileAuthState::Authed(_));
                    if !is_authed {
                        profile.auth_data = ProfileAuthState::None;
                    }
                }
                if !is_authed {
                    self.try_auth_current_active().await?;
                }

                let client;
                let is_authed;
                let main_secret;
                {
                    let mut profiles = self.profiles.lock();
                    let profile = profiles
                        .profiles
                        .get_mut(&cur_profile)
                        .ok_or_else(|| anyhow!("no current profile active."))?;
                    client = profile.client.clone();

                    (is_authed, main_secret) =
                        if let ProfileAuthState::Authed(auth) = &profile.auth_data {
                            (true, auth.main_secret.clone())
                        } else {
                            (false, Vec::new())
                        };
                    drop(profiles);
                }
                if is_authed {
                    let res = account_client::connect_game_server::connect_game_server(
                        game_server_group_id,
                        &main_secret,
                        client.as_ref(),
                    )
                    .await?;
                    Ok(res)
                } else {
                    Err(anyhow!("Not authed."))
                }
            }
            GameServerMode::Ip(ip) => {
                let ip_path = ip.to_string().replace('.', "_").replace(':', "-");
                let ip_path = PathBuf::from_str("offline")?.join(ip_path);
                // ignore error, not critical here
                let _ = self.fs.create_dirs("offline".as_ref()).await;
                match self
                    .fs
                    .read(ip_path.as_ref())
                    .await
                    .map_err(|err| anyhow!(err))
                    .and_then(|key_pair| {
                        serde_json::from_slice(&key_pair).map_err(|err| anyhow!(err))
                    }) {
                    Ok(key_pair) => Ok(key_pair),
                    Err(_) => {
                        let key_pair = generate_client_game_server_key_pair()?;

                        // ignore error, not critical here
                        let _ = self
                            .fs
                            .write(
                                ip_path.as_ref(),
                                serde_json::to_string(&key_pair)?.into_bytes(),
                            )
                            .await;

                        Ok(key_pair)
                    }
                }
            }
        }
    }

    /// The game server wants it's game server group key.
    pub async fn get_game_server_group_data(
        &self,
        email: email_address::EmailAddress,
        password: &str,
    ) -> anyhow::Result<GameServerGroupData> {
        let cur_profile = Self::email_to_path_friendy(&email);
        let is_authed;
        let client;
        {
            let mut profiles = self.profiles.lock();
            let profile = profiles
                .profiles
                .get_mut(&cur_profile)
                .ok_or_else(|| anyhow!("no current profile active."))?;

            is_authed = if let ProfileAuthState::Authed(auth) = &profile.auth_data {
                Some(auth.main_secret.clone())
            } else {
                None
            };
            if is_authed.is_none() {
                profile.auth_data = ProfileAuthState::None;
            }
            client = profile.client.clone();
        }
        let main_secret = match is_authed {
            Some(auth) => auth,
            None => {
                match account_client::login::login_offline(password, &*client).await {
                    Ok(login) => Ok(login),
                    Err(_) => account_client::login::login(email, password, &*client)
                        .await
                        .map_err(|err| AuthResult::Other(err.into()))
                        .and_then(|login_res| {
                            if let AuthLoginResult::Verified(auth) = login_res.auth {
                                Ok(*auth)
                            } else {
                                Err(AuthResult::AccountNotVerified)
                            }
                        }),
                }?
                .main_secret
            }
        };

        let res = account_client::game_server_group_data::get_game_server_group_data(
            &main_secret,
            client.as_ref(),
        )
        .await?;
        Ok(res)
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
                    auth_data: Default::default(),
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
