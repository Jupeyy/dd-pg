use std::{collections::VecDeque, sync::Arc, thread::ThreadId};

use api::{graphics::graphics::GraphicsBackend, println, GRAPHICS, IO, RUNTIME_THREAD_POOL};
use api_ui_game::render::create_skin_container;
use base_io::{io::Io, io_batcher::IoBatcherTask, yield_now::yield_now};
use client_containers::skins::SkinContainer;
use client_render_base::render::tee::RenderTee;
use client_types::{
    chat::{ChatMsg, ChatMsgPlayerChannel, ServerMsg},
    console::ConsoleEntry,
    server_browser::{
        ServerBrowserData, ServerBrowserFilter, ServerBrowserInfo, ServerBrowserInfoMap,
        ServerBrowserServer,
    },
};
use client_ui::{
    client_info::ClientInfo,
    console::user_data::UserData,
    events::UiEvents,
    main_menu::{
        constants::{MENU_PROFILE_NAME, MENU_SETTINGS_NAME, MENU_UI_PAGE_QUERY},
        page::MainMenuUi,
        profiles_interface::ProfilesInterface,
        settings::constants::SETTINGS_UI_PAGE_QUERY,
        user_data::{MainMenuInterface, UiMonitors},
    },
};
use game_config::config::Config;
use graphics::{graphics::graphics::Graphics, handles::canvas::canvas::GraphicsCanvasHandle};
use shared_base::network::server_info::ServerInfo;
use ui_base::types::{UiRenderPipe, UiState};
use ui_traits::traits::UiPageInterface;

use super::profiles::Profiles;

struct MenuImpl {}

impl MainMenuInterface for MenuImpl {
    fn refresh(&mut self) {}
}

pub struct MainMenu {
    config: Config,
    selected_index: Option<usize>,
    canvas_handle: GraphicsCanvasHandle,
    skin_container: SkinContainer,
    render_tee: RenderTee,
    browser_data: ServerBrowserData,
    io: Io,
    servers: Option<IoBatcherTask<String>>,
}

impl MainMenu {
    pub fn new(graphics: &Graphics, io: Io) -> Self {
        let mut config = Config::default();
        config.engine.ui.path.query.insert(
            MENU_UI_PAGE_QUERY.to_string(),
            MENU_PROFILE_NAME.to_string(),
        );
        config
            .engine
            .ui
            .path
            .query
            .insert(SETTINGS_UI_PAGE_QUERY.to_string(), "General".to_string());

        let mut servers = Vec::new();
        for i in 0..100 {
            servers.push(ServerBrowserServer {
                info: ServerBrowserInfo {
                    name: format!("demo_server {i}"),
                    game_type: format!("demo_server {i}"),
                    version: format!("demo_version {i}"),
                    map: ServerBrowserInfoMap {
                        name: format!("demo_server {i}"),
                        sha256: format!("demo_server {i}"),
                        size: 0,
                    },
                    players: Vec::new(),
                    passworded: false,
                },
                address: "127.0.0.1:8303".into(),
            });
        }

        let http = io.http.clone();
        let servers_task: IoBatcherTask<String> = io
            .io_batcher
            .spawn(async move {
                Ok(http
                    .download_text(
                        "https://master1.ddnet.org/ddnet/15/servers.json"
                            .try_into()
                            .unwrap(),
                    )
                    .await?)
            })
            .cancelable();

        Self {
            config,
            selected_index: None,
            canvas_handle: graphics.canvas_handle.clone(),
            skin_container: create_skin_container(),
            render_tee: RenderTee::new(graphics),

            browser_data: ServerBrowserData {
                servers,
                filter: ServerBrowserFilter {
                    search: "".to_string(),
                    exclude: "".to_string(),
                },
                cur_address: "127.0.0.1:8303".to_string(),
            },
            io,
            servers: Some(servers_task),
        }
    }

    fn render_impl(
        &mut self,
        ui: &mut egui::Ui,
        pipe: &mut UiRenderPipe<Config>,
        ui_state: &mut UiState,
        main_frame_only: bool,
    ) {
        client_ui::main_menu::main_frame::render(
            ui,
            &mut UiRenderPipe::new(
                pipe.cur_time,
                &mut client_ui::main_menu::user_data::UserData {
                    browser_data: &mut self.browser_data,
                    server_info: &Default::default(),
                    render_options: client_ui::main_menu::user_data::RenderOptions {
                        hide_buttons_icons: false,
                    },
                    main_menu: &mut MenuImpl {},
                    config: &mut self.config,
                    events: &UiEvents::new(),
                    client_info: &ClientInfo::default(),
                    selected_index: &mut self.selected_index,
                    canvas_handle: &self.canvas_handle,
                    render_tee: &self.render_tee,
                    skin_container: &mut self.skin_container,
                    full_rect: ui.available_rect_before_wrap(),
                    profiles: &{
                        let profiles: Arc<dyn ProfilesInterface> = Arc::new(Profiles);
                        profiles
                    },
                    profile_tasks: &mut Default::default(),
                    io: &*unsafe { IO.borrow() },
                    monitors: &UiMonitors::new(Vec::new()),
                },
            ),
            ui_state,
            main_frame_only,
        );
    }
}

impl UiPageInterface<Config> for MainMenu {
    fn has_blur(&self) -> bool {
        true
    }

    fn render_main_frame(
        &mut self,
        ui: &mut egui::Ui,
        pipe: &mut UiRenderPipe<Config>,
        ui_state: &mut UiState,
    ) {
        self.render_impl(ui, pipe, ui_state, true)
    }

    fn render(
        &mut self,
        ui: &mut egui::Ui,
        pipe: &mut UiRenderPipe<Config>,
        ui_state: &mut UiState,
    ) {
        if self
            .servers
            .as_ref()
            .is_some_and(|servers| servers.is_finished())
        {
            let servers_raw = self.servers.take().unwrap();
            self.browser_data.servers =
                MainMenuUi::json_to_server_browser(servers_raw.get_storage().unwrap().as_str());
        }

        self.render_impl(ui, pipe, ui_state, false)
    }

    fn unmount(&mut self) {
        self.skin_container.clear_except_default();
    }
}
