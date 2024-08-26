use std::sync::Arc;

use base_io::{io::Io, io_batcher::IoBatcherTask};
use client_containers::skins::{SkinContainer, SKIN_CONTAINER_PATH};
use client_render_base::render::tee::RenderTee;
use shared_base::server_browser::{
    ServerBrowserData, ServerBrowserFilter, ServerBrowserInfo, ServerBrowserServer,
};

use game_config::config::Config;
use graphics::{graphics::graphics::Graphics, handles::canvas::canvas::GraphicsCanvasHandle};
use master_server_types::{addr::Protocol, servers::BrowserServers};
use shared_base::network::server_info::ServerInfo;
use sound::sound::SoundManager;
use ui_base::types::{UiRenderPipe, UiState};
use ui_traits::traits::UiPageInterface;

use crate::{client_info::ClientInfo, events::UiEvents, main_menu::user_data::MainMenuInterface};

use super::{
    main_frame,
    profiles_interface::ProfilesInterface,
    user_data::{ProfileTasks, RenderOptions, UiMonitors, UserData},
};

pub struct MainMenuIo {
    pub(crate) io: Io,
    cur_servers_task: Option<IoBatcherTask<String>>,
}

impl MainMenuInterface for MainMenuIo {
    fn refresh(&mut self) {
        self.cur_servers_task = Some(MainMenuUi::req_server_list(&self.io));
    }
}

pub struct MainMenuUi {
    pub(crate) server_info: Arc<ServerInfo>,
    pub(crate) client_info: ClientInfo,
    pub(crate) browser_data: ServerBrowserData,

    menu_io: MainMenuIo,
    io: Io,
    events: UiEvents,

    selected_index: Option<usize>,

    pub canvas_handle: GraphicsCanvasHandle,
    pub skin_container: SkinContainer,
    pub render_tee: RenderTee,

    pub profiles: Arc<dyn ProfilesInterface>,
    pub profile_tasks: ProfileTasks,

    pub monitors: UiMonitors,
}

impl MainMenuUi {
    fn req_server_list(io: &Io) -> IoBatcherTask<String> {
        let http = io.http.clone();
        io.io_batcher
            .spawn(async move {
                Ok(http
                    .download_text(
                        "https://pg.ddnet.org:4444/ddnet/15/servers.json"
                            .try_into()
                            .unwrap(),
                    )
                    .await?)
            })
            .cancelable()
    }

    pub fn new(
        graphics: &Graphics,
        sound: &SoundManager,
        server_info: Arc<ServerInfo>,
        client_info: ClientInfo,
        events: UiEvents,
        io: Io,
        tp: Arc<rayon::ThreadPool>,
        profiles: Arc<dyn ProfilesInterface>,
        monitors: UiMonitors,
    ) -> Self {
        let cur_servers_task = Self::req_server_list(&io);

        let mut profile_tasks: ProfileTasks = Default::default();
        let profiles_task = profiles.clone();
        profile_tasks.user_interactions.push(
            io.io_batcher
                .spawn(async move { profiles_task.user_interaction().await })
                .cancelable(),
        );

        Self {
            server_info,
            client_info,
            browser_data: ServerBrowserData {
                servers: Vec::new(),
                filter: ServerBrowserFilter {
                    exclude: Default::default(),
                    search: Default::default(),
                },
                cur_address: Default::default(),
                cur_cert_hash: None,
            },
            menu_io: MainMenuIo {
                io: io.clone(),
                cur_servers_task: Some(cur_servers_task),
            },
            io: io.clone(),
            events,
            selected_index: None,

            canvas_handle: graphics.canvas_handle.clone(),
            skin_container: {
                let default_skin = SkinContainer::load_default(&io, SKIN_CONTAINER_PATH.as_ref());
                let scene = sound.scene_handle.create();
                SkinContainer::new(
                    io.clone(),
                    tp,
                    default_skin,
                    None,
                    None,
                    "skin-container",
                    graphics,
                    sound,
                    &scene,
                    SKIN_CONTAINER_PATH.as_ref(),
                )
            },
            render_tee: RenderTee::new(graphics),
            profiles,
            profile_tasks,
            monitors,
        }
    }

    pub(crate) fn get_user_data<'a>(
        &'a mut self,
        config: &'a mut Config,
        hide_buttons_right: bool,
        ui: &egui::Ui,
    ) -> UserData<'a> {
        UserData {
            server_info: &self.server_info,
            client_info: &self.client_info,
            browser_data: &mut self.browser_data,
            render_options: RenderOptions {
                hide_buttons_icons: hide_buttons_right,
            },

            main_menu: &mut self.menu_io,
            config,
            events: &self.events,
            selected_index: &mut self.selected_index,
            canvas_handle: &self.canvas_handle,
            render_tee: &self.render_tee,
            skin_container: &mut self.skin_container,
            full_rect: ui.available_rect_before_wrap(),
            profiles: &self.profiles,
            profile_tasks: &mut self.profile_tasks,
            io: &self.io,
            monitors: &self.monitors,
        }
    }

    pub fn json_to_server_browser(servers_raw: &str) -> Vec<ServerBrowserServer> {
        let servers: BrowserServers = match serde_json::from_str(servers_raw) {
            Ok(servers) => servers,
            Err(err) => {
                log::error!("could not parse servers json: {err}");
                return Default::default();
            }
        };

        let parsed_servers: Vec<ServerBrowserServer> = servers
            .servers
            .into_iter()
            .filter_map(|server| {
                if let Some(addr) = server
                    .addresses
                    .iter()
                    .find(|addr| addr.protocol == Protocol::VPg)
                {
                    let info: serde_json::Result<ServerBrowserInfo> =
                        serde_json::from_str(server.info.get());
                    match info {
                        Ok(info) => Some(ServerBrowserServer {
                            address: addr.ip.to_string() + ":" + &addr.port.to_string(),
                            info,
                        }),
                        Err(err) => {
                            log::error!("ServerBrowserInfo could not be parsed: {err}");
                            None
                        }
                    }
                } else {
                    None
                }
            })
            .collect();
        parsed_servers
    }
}

impl UiPageInterface<Config> for MainMenuUi {
    fn has_blur(&self) -> bool {
        true
    }

    fn render_main_frame(
        &mut self,
        ui: &mut egui::Ui,
        pipe: &mut UiRenderPipe<Config>,
        ui_state: &mut UiState,
    ) {
        main_frame::render(
            ui,
            &mut UiRenderPipe {
                cur_time: pipe.cur_time,
                user_data: &mut self.get_user_data(pipe.user_data, false, ui),
            },
            ui_state,
            true,
        );
    }

    fn render(
        &mut self,
        ui: &mut egui::Ui,
        pipe: &mut UiRenderPipe<Config>,
        ui_state: &mut UiState,
    ) {
        if let Some(server_task) = &self.menu_io.cur_servers_task {
            if server_task.is_finished() {
                match self.menu_io.cur_servers_task.take().unwrap().get_storage() {
                    Ok(servers_raw) => {
                        self.browser_data.servers = Self::json_to_server_browser(&servers_raw);
                    }
                    Err(err) => {
                        log::error!("failed to download master server list: {err}");
                    }
                }
            }
        }

        main_frame::render(
            ui,
            &mut UiRenderPipe {
                cur_time: pipe.cur_time,
                user_data: &mut self.get_user_data(pipe.user_data, false, ui),
            },
            ui_state,
            false,
        )
    }

    fn unmount(&mut self) {
        self.skin_container.clear_except_default();
        self.profile_tasks = Default::default();
        self.menu_io.cur_servers_task = None;
    }
}
