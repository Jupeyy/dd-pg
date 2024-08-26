use std::{rc::Rc, sync::Arc};

use base_io::{io::Io, io_batcher::IoBatcherTask};
use client_containers::skins::SkinContainer;
use client_render_base::render::tee::RenderTee;
use game_config::config::Config;
use graphics::handles::canvas::canvas::GraphicsCanvasHandle;
use hiarc::{hiarc_safer_rc_refcell, Hiarc};
use shared_base::network::server_info::ServerInfo;
use shared_base::server_browser::ServerBrowserData;

use crate::{client_info::ClientInfo, events::UiEvents};

use super::profiles_interface::ProfilesInterface;

#[derive(Debug, Clone, Copy)]
pub struct RenderOptions {
    pub hide_buttons_icons: bool,
}

pub trait MainMenuInterface {
    fn refresh(&mut self);
}

#[derive(Debug, Default)]
pub struct ProfileTasks {
    pub login_tokens: Vec<IoBatcherTask<()>>,
    pub logins: Vec<IoBatcherTask<()>>,
    pub user_interactions: Vec<IoBatcherTask<()>>,
    pub errors: Vec<String>,
}

impl ProfileTasks {
    pub fn update(&mut self) {
        let mut handle_task = |tasks: &mut Vec<IoBatcherTask<()>>| {
            let login = std::mem::take(tasks);
            for login in login.into_iter() {
                if login.is_finished() {
                    if let Err(err) = login.get_storage() {
                        self.errors.push(err.to_string());
                    }
                } else {
                    tasks.push(login);
                }
            }
        };
        handle_task(&mut self.login_tokens);
        handle_task(&mut self.logins);
        handle_task(&mut self.user_interactions);
    }
}

#[derive(Debug, Hiarc)]
pub struct UiMonitorVideoMode {
    pub width: u32,
    pub height: u32,
    pub refresh_rate_mhz: u32,
}

#[derive(Debug, Hiarc)]
pub struct UiMonitor {
    pub name: String,
    pub video_modes: Vec<UiMonitorVideoMode>,
}

#[hiarc_safer_rc_refcell]
#[derive(Debug, Hiarc)]
pub struct UiMonitors {
    monitors: Rc<Vec<UiMonitor>>,
}

#[hiarc_safer_rc_refcell]
impl UiMonitors {
    pub fn new(monitors: Vec<UiMonitor>) -> Self {
        Self {
            monitors: Rc::new(monitors),
        }
    }

    pub fn monitors(&self) -> Rc<Vec<UiMonitor>> {
        self.monitors.clone()
    }
}

pub struct UserData<'a> {
    pub browser_data: &'a mut ServerBrowserData,
    pub server_info: &'a Arc<ServerInfo>,
    pub selected_index: &'a mut Option<usize>,

    pub render_options: RenderOptions,

    pub main_menu: &'a mut dyn MainMenuInterface,

    pub config: &'a mut Config,

    pub events: &'a UiEvents,
    pub client_info: &'a ClientInfo,

    pub canvas_handle: &'a GraphicsCanvasHandle,
    pub skin_container: &'a mut SkinContainer,
    pub render_tee: &'a RenderTee,

    pub profiles: &'a Arc<dyn ProfilesInterface>,
    pub profile_tasks: &'a mut ProfileTasks,
    pub io: &'a Io,

    pub full_rect: egui::Rect,

    pub monitors: &'a UiMonitors,
}
