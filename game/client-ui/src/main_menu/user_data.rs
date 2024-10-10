use std::{path::Path, sync::Arc};

use base_io::{io::Io, io_batcher::IoBatcherTask};
use client_containers::{
    ctf::CtfContainer, emoticons::EmoticonsContainer, entities::EntitiesContainer,
    flags::FlagsContainer, freezes::FreezeContainer, game::GameContainer, hooks::HookContainer,
    hud::HudContainer, ninja::NinjaContainer, particles::ParticlesContainer, skins::SkinContainer,
    weapons::WeaponContainer,
};
use client_render_base::{
    map::{map_buffered::TileLayerVisuals, map_pipeline::MapGraphics},
    render::{tee::RenderTee, toolkit::ToolkitRender},
};
use demo::{DemoHeader, DemoHeaderExt};
use game_config::config::Config;
use graphics::{
    graphics_mt::GraphicsMultiThreaded,
    handles::{
        backend::backend::GraphicsBackendHandle, canvas::canvas::GraphicsCanvasHandle,
        stream::stream::GraphicsStreamHandle, texture::texture::GraphicsTextureHandle,
    },
};
use shared_base::network::server_info::ServerInfo;
use shared_base::server_browser::ServerBrowserData;
use url::Url;

use crate::{client_info::ClientInfo, events::UiEvents};

use super::{
    communities::CommunityIcons,
    ddnet_info::DdnetInfo,
    demo_list::DemoList,
    monitors::UiMonitors,
    player_settings_ntfy::PlayerSettingsSync,
    profiles_interface::{
        AccountInfo, AccountTokenError, CredentialAuthTokenError, ProfileData, ProfilesInterface,
    },
    spatial_chat::SpatialChat,
    theme_container::ThemeContainer,
};

#[derive(Debug, Clone, Copy)]
pub struct RenderOptions {
    pub hide_buttons_icons: bool,
}

pub trait MainMenuInterface {
    fn refresh(&mut self);

    fn refresh_demo_list(&mut self, path: &Path);
    fn refresh_demo_info(&mut self, file: &Path);
}

#[derive(Debug, Clone)]
pub enum AccountCredential {
    Email,
    Steam,
}

#[derive(Debug, Clone)]
pub enum CredentialAuthOperation {
    Login,
    LinkCredential {
        profile_name: String,
        account_credential: AccountCredential,
    },
    UnlinkCredential {
        profile_name: String,
    },
}

#[derive(Debug, Clone)]
pub enum AccountOperation {
    LogoutAll {
        profile_name: String,
    },
    LinkCredential {
        credential_auth_token: String,
        profile_name: String,
    },
    Delete {
        profile_name: String,
    },
}

#[derive(Debug, Default)]
pub enum ProfileState {
    #[default]
    Overview,

    EmailCredentialAuthTokenPrepare(CredentialAuthOperation),
    EmailCredentialAuthToken {
        op: CredentialAuthOperation,
        task: IoBatcherTask<Result<(), CredentialAuthTokenError>>,
    },
    EmailCredentialAuthTokenObtained(CredentialAuthOperation),
    EmailCredentialAuthTokenWebValidation {
        op: CredentialAuthOperation,
        url: Url,
    },

    EmailLoggingIn(IoBatcherTask<()>),
    EmailUnlinkCredential(IoBatcherTask<()>),

    SteamCredentialAuthTokenPrepare(CredentialAuthOperation),
    SteamCredentialAuthToken {
        op: CredentialAuthOperation,
        task: IoBatcherTask<Result<String, CredentialAuthTokenError>>,
    },
    SteamCredentialAuthTokenObtained {
        op: CredentialAuthOperation,
        token: String,
    },
    SteamCredentialAuthTokenWebValidation {
        op: CredentialAuthOperation,
        url: Url,
    },

    SteamLoggingIn(IoBatcherTask<()>),
    SteamUnlinkCredential(IoBatcherTask<()>),

    EmailAccountTokenPrepare(AccountOperation),
    EmailAccountToken {
        op: AccountOperation,
        task: IoBatcherTask<Result<(), AccountTokenError>>,
    },
    EmailAccountTokenObtained(AccountOperation),
    EmailAccountTokenWebValidation {
        op: AccountOperation,
        url: Url,
    },

    EmailLogoutAll(IoBatcherTask<()>),
    EmailLinkCredential(IoBatcherTask<()>),
    EmailDelete(IoBatcherTask<()>),

    SteamAccountTokenPrepare(AccountOperation),
    SteamAccountToken {
        op: AccountOperation,
        task: IoBatcherTask<Result<String, AccountTokenError>>,
    },
    SteamAccountTokenObtained {
        op: AccountOperation,
        token: String,
    },
    SteamAccountTokenWebValidation {
        op: AccountOperation,
        url: Url,
    },

    SteamLogoutAll(IoBatcherTask<()>),
    SteamLinkCredential(IoBatcherTask<()>),
    SteamDelete(IoBatcherTask<()>),

    AccountInfoFetch {
        task: IoBatcherTask<AccountInfo>,
        profile_name: String,
        profile_data: ProfileData,
    },
    AccountInfo {
        info: AccountInfo,
        profile_name: String,
        profile_data: ProfileData,
    },

    Logout(IoBatcherTask<()>),
    LogoutAllPrepare {
        profile_name: String,
        info: AccountInfo,
    },
    DeleteConfirm {
        profile_name: String,
        info: AccountInfo,
    },
    DeletePrepare {
        profile_name: String,
        info: AccountInfo,
    },
    LinkEmailPrepare {
        profile_name: String,
        info: AccountInfo,
    },
    LinkSteamPrepare {
        profile_name: String,
        info: AccountInfo,
    },
    UnlinkEmailPrepare {
        profile_name: String,
        info: AccountInfo,
    },
    UnlinkSteamPrepare {
        profile_name: String,
        info: AccountInfo,
    },

    Err(String),
}

#[derive(Debug, Default)]
pub struct ProfileTasks {
    pub state: ProfileState,
    pub user_interactions: Vec<IoBatcherTask<()>>,
}

impl ProfileTasks {
    pub fn update(&mut self) {
        self.state = match std::mem::take(&mut self.state) {
            ProfileState::Overview => ProfileState::Overview,

            ProfileState::EmailCredentialAuthTokenPrepare(op) => {
                ProfileState::EmailCredentialAuthTokenPrepare(op)
            }
            ProfileState::EmailCredentialAuthToken { op, task } => {
                if task.is_finished() {
                    match task.get_storage() {
                        Ok(res) => match res {
                            Ok(_) => ProfileState::EmailCredentialAuthTokenObtained(op),
                            Err(err) => match err {
                                CredentialAuthTokenError::WebValidationProcessNeeded { url } => {
                                    ProfileState::EmailCredentialAuthTokenWebValidation { op, url }
                                }
                                CredentialAuthTokenError::Other(err) => {
                                    ProfileState::Err(err.to_string())
                                }
                            },
                        },
                        Err(err) => ProfileState::Err(err.to_string()),
                    }
                } else {
                    ProfileState::EmailCredentialAuthToken { op, task }
                }
            }
            ProfileState::EmailCredentialAuthTokenObtained(op) => {
                ProfileState::EmailCredentialAuthTokenObtained(op)
            }
            ProfileState::EmailCredentialAuthTokenWebValidation { op, url } => {
                ProfileState::EmailCredentialAuthTokenWebValidation { op, url }
            }

            ProfileState::EmailLoggingIn(task) => {
                if task.is_finished() {
                    match task.get_storage() {
                        Ok(_) => ProfileState::Overview,
                        Err(err) => ProfileState::Err(err.to_string()),
                    }
                } else {
                    ProfileState::EmailLoggingIn(task)
                }
            }
            ProfileState::EmailLinkCredential(task) => {
                if task.is_finished() {
                    match task.get_storage() {
                        Ok(_) => ProfileState::Overview,
                        Err(err) => ProfileState::Err(err.to_string()),
                    }
                } else {
                    ProfileState::EmailLinkCredential(task)
                }
            }
            ProfileState::EmailUnlinkCredential(task) => {
                if task.is_finished() {
                    match task.get_storage() {
                        Ok(_) => ProfileState::Overview,
                        Err(err) => ProfileState::Err(err.to_string()),
                    }
                } else {
                    ProfileState::EmailUnlinkCredential(task)
                }
            }
            ProfileState::EmailLogoutAll(task) => {
                if task.is_finished() {
                    match task.get_storage() {
                        Ok(_) => ProfileState::Overview,
                        Err(err) => ProfileState::Err(err.to_string()),
                    }
                } else {
                    ProfileState::EmailLogoutAll(task)
                }
            }
            ProfileState::EmailDelete(task) => {
                if task.is_finished() {
                    match task.get_storage() {
                        Ok(_) => ProfileState::Overview,
                        Err(err) => ProfileState::Err(err.to_string()),
                    }
                } else {
                    ProfileState::EmailDelete(task)
                }
            }

            ProfileState::SteamCredentialAuthTokenPrepare(op) => {
                ProfileState::SteamCredentialAuthTokenPrepare(op)
            }
            ProfileState::SteamCredentialAuthToken { op, task } => {
                if task.is_finished() {
                    match task.get_storage() {
                        Ok(res) => match res {
                            Ok(token) => {
                                ProfileState::SteamCredentialAuthTokenObtained { op, token }
                            }
                            Err(err) => match err {
                                CredentialAuthTokenError::WebValidationProcessNeeded { url } => {
                                    ProfileState::SteamCredentialAuthTokenWebValidation { op, url }
                                }
                                CredentialAuthTokenError::Other(err) => {
                                    ProfileState::Err(err.to_string())
                                }
                            },
                        },
                        Err(err) => ProfileState::Err(err.to_string()),
                    }
                } else {
                    ProfileState::SteamCredentialAuthToken { op, task }
                }
            }
            ProfileState::SteamCredentialAuthTokenObtained { op, token } => {
                ProfileState::SteamCredentialAuthTokenObtained { op, token }
            }
            ProfileState::SteamCredentialAuthTokenWebValidation { op, url } => {
                ProfileState::SteamCredentialAuthTokenWebValidation { op, url }
            }

            ProfileState::SteamLoggingIn(task) => {
                if task.is_finished() {
                    match task.get_storage() {
                        Ok(_) => ProfileState::Overview,
                        Err(err) => ProfileState::Err(err.to_string()),
                    }
                } else {
                    ProfileState::SteamLoggingIn(task)
                }
            }
            ProfileState::SteamLinkCredential(task) => {
                if task.is_finished() {
                    match task.get_storage() {
                        Ok(_) => ProfileState::Overview,
                        Err(err) => ProfileState::Err(err.to_string()),
                    }
                } else {
                    ProfileState::SteamLinkCredential(task)
                }
            }
            ProfileState::SteamUnlinkCredential(task) => {
                if task.is_finished() {
                    match task.get_storage() {
                        Ok(_) => ProfileState::Overview,
                        Err(err) => ProfileState::Err(err.to_string()),
                    }
                } else {
                    ProfileState::SteamUnlinkCredential(task)
                }
            }
            ProfileState::SteamLogoutAll(task) => {
                if task.is_finished() {
                    match task.get_storage() {
                        Ok(_) => ProfileState::Overview,
                        Err(err) => ProfileState::Err(err.to_string()),
                    }
                } else {
                    ProfileState::SteamLogoutAll(task)
                }
            }
            ProfileState::SteamDelete(task) => {
                if task.is_finished() {
                    match task.get_storage() {
                        Ok(_) => ProfileState::Overview,
                        Err(err) => ProfileState::Err(err.to_string()),
                    }
                } else {
                    ProfileState::SteamDelete(task)
                }
            }

            ProfileState::EmailAccountTokenPrepare(op) => {
                ProfileState::EmailAccountTokenPrepare(op)
            }
            ProfileState::EmailAccountToken { op, task } => {
                if task.is_finished() {
                    match task.get_storage() {
                        Ok(res) => match res {
                            Ok(_) => ProfileState::EmailAccountTokenObtained(op),
                            Err(err) => match err {
                                AccountTokenError::WebValidationProcessNeeded { url } => {
                                    ProfileState::EmailAccountTokenWebValidation { op, url }
                                }
                                AccountTokenError::Other(err) => ProfileState::Err(err.to_string()),
                            },
                        },
                        Err(err) => ProfileState::Err(err.to_string()),
                    }
                } else {
                    ProfileState::EmailAccountToken { op, task }
                }
            }
            ProfileState::EmailAccountTokenObtained(op) => {
                ProfileState::EmailAccountTokenObtained(op)
            }
            ProfileState::EmailAccountTokenWebValidation { op, url } => {
                ProfileState::EmailAccountTokenWebValidation { op, url }
            }

            ProfileState::SteamAccountTokenPrepare(op) => {
                ProfileState::SteamAccountTokenPrepare(op)
            }
            ProfileState::SteamAccountToken { op, task } => {
                if task.is_finished() {
                    match task.get_storage() {
                        Ok(res) => match res {
                            Ok(token) => ProfileState::SteamAccountTokenObtained { op, token },
                            Err(err) => match err {
                                AccountTokenError::WebValidationProcessNeeded { url } => {
                                    ProfileState::SteamAccountTokenWebValidation { op, url }
                                }
                                AccountTokenError::Other(err) => ProfileState::Err(err.to_string()),
                            },
                        },
                        Err(err) => ProfileState::Err(err.to_string()),
                    }
                } else {
                    ProfileState::SteamAccountToken { op, task }
                }
            }
            ProfileState::SteamAccountTokenObtained { op, token } => {
                ProfileState::SteamAccountTokenObtained { op, token }
            }
            ProfileState::SteamAccountTokenWebValidation { op, url } => {
                ProfileState::SteamAccountTokenWebValidation { op, url }
            }

            ProfileState::AccountInfoFetch {
                task,
                profile_name,
                profile_data,
            } => {
                if task.is_finished() {
                    match task.get_storage() {
                        Ok(info) => ProfileState::AccountInfo {
                            info,
                            profile_name,
                            profile_data,
                        },
                        Err(err) => ProfileState::Err(err.to_string()),
                    }
                } else {
                    ProfileState::AccountInfoFetch {
                        task,
                        profile_name,
                        profile_data,
                    }
                }
            }
            ProfileState::AccountInfo {
                info,
                profile_name,
                profile_data,
            } => ProfileState::AccountInfo {
                info,
                profile_name,
                profile_data,
            },
            ProfileState::Logout(task) => {
                if task.is_finished() {
                    match task.get_storage() {
                        Ok(_) => ProfileState::Overview,
                        Err(err) => ProfileState::Err(err.to_string()),
                    }
                } else {
                    ProfileState::Logout(task)
                }
            }
            ProfileState::LogoutAllPrepare { profile_name, info } => {
                ProfileState::LogoutAllPrepare { profile_name, info }
            }
            ProfileState::DeleteConfirm { profile_name, info } => {
                ProfileState::DeleteConfirm { profile_name, info }
            }
            ProfileState::DeletePrepare { profile_name, info } => {
                ProfileState::DeletePrepare { profile_name, info }
            }
            ProfileState::LinkEmailPrepare { profile_name, info } => {
                ProfileState::LinkEmailPrepare { profile_name, info }
            }
            ProfileState::LinkSteamPrepare { profile_name, info } => {
                ProfileState::LinkSteamPrepare { profile_name, info }
            }
            ProfileState::UnlinkEmailPrepare { profile_name, info } => {
                ProfileState::UnlinkEmailPrepare { profile_name, info }
            }
            ProfileState::UnlinkSteamPrepare { profile_name, info } => {
                ProfileState::UnlinkSteamPrepare { profile_name, info }
            }
            ProfileState::Err(err) => ProfileState::Err(err),
        };

        fn handle_task<T>(tasks: &mut Vec<IoBatcherTask<T>>) -> Vec<T> {
            let tasks_dummy = std::mem::take(tasks);
            let mut res = Vec::new();
            for task in tasks_dummy.into_iter() {
                if task.is_finished() {
                    match task.get_storage() {
                        Ok(t) => {
                            res.push(t);
                        }
                        Err(err) => {
                            log::error!("{err}");
                        }
                    }
                } else {
                    tasks.push(task);
                }
            }
            res
        }

        handle_task(&mut self.user_interactions);
    }
}

pub struct UserData<'a> {
    pub browser_data: &'a mut ServerBrowserData,
    pub server_info: &'a Arc<ServerInfo>,

    pub ddnet_info: &'a DdnetInfo,
    pub icons: &'a mut CommunityIcons,

    pub demos: &'a DemoList,
    pub demo_info: &'a Option<(DemoHeader, DemoHeaderExt)>,

    pub render_options: RenderOptions,

    pub main_menu: &'a mut dyn MainMenuInterface,

    pub config: &'a mut Config,

    pub events: &'a UiEvents,
    pub client_info: &'a ClientInfo,

    pub spatial_chat: &'a SpatialChat,
    pub player_settings_sync: &'a PlayerSettingsSync,

    pub texture_handle: &'a GraphicsTextureHandle,
    pub backend_handle: &'a GraphicsBackendHandle,
    pub stream_handle: &'a GraphicsStreamHandle,
    pub canvas_handle: &'a GraphicsCanvasHandle,
    pub graphics_mt: &'a GraphicsMultiThreaded,

    pub skin_container: &'a mut SkinContainer,
    pub render_tee: &'a RenderTee,
    pub flags_container: &'a mut FlagsContainer,
    pub toolkit_render: &'a ToolkitRender,
    pub weapons_container: &'a mut WeaponContainer,
    pub hook_container: &'a mut HookContainer,
    pub entities_container: &'a mut EntitiesContainer,
    pub freeze_container: &'a mut FreezeContainer,
    pub emoticons_container: &'a mut EmoticonsContainer,
    pub particles_container: &'a mut ParticlesContainer,
    pub ninja_container: &'a mut NinjaContainer,
    pub game_container: &'a mut GameContainer,
    pub hud_container: &'a mut HudContainer,
    pub ctf_container: &'a mut CtfContainer,
    pub theme_container: &'a mut ThemeContainer,

    pub map_render: &'a MapGraphics,
    pub tile_set_preview: &'a TileLayerVisuals,

    pub profiles: &'a Arc<dyn ProfilesInterface>,
    pub profile_tasks: &'a mut ProfileTasks,
    pub io: &'a Io,

    pub full_rect: egui::Rect,

    pub monitors: &'a UiMonitors,
}
