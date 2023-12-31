use std::sync::Arc;

use base_io::{io::IO, io_batcher::IOBatcherTask};
use client_types::server_browser::{
    ServerBrowserData, ServerBrowserFilter, ServerBrowserInfo, ServerBrowserServer,
};
use graphics::graphics::Graphics;
use master_server_types::{addr::Protocol, servers::BrowserServers};
use shared_base::network::server_info::ServerInfo;
use ui_base::types::{UIPipe, UIState};
use ui_traits::traits::UIRenderCallbackFunc;

use crate::main_menu::user_data::MainMenuInterface;

use super::{
    main_frame,
    user_data::{RenderOptions, UserData},
};

pub struct MainMenuIO {
    pub(crate) io: IO,
    cur_servers_task: Option<IOBatcherTask<String>>,
}

impl MainMenuInterface for MainMenuIO {
    fn refresh(&mut self) {
        self.cur_servers_task = Some(MainMenuUI::req_server_list(&self.io));
    }
}

pub struct MainMenuUI {
    pub(crate) server_info: Arc<ServerInfo>,
    pub(crate) browser_data: ServerBrowserData,

    menu_io: MainMenuIO,
}

impl MainMenuUI {
    fn req_server_list(io: &IO) -> IOBatcherTask<String> {
        let http = io.http.clone();
        io.io_batcher
            .spawn(async move {
                http.download_text("https://master1.ddnet.org/ddnet/15/servers.json")
                    .await
            })
            .cancelable()
    }

    pub fn new(server_info: Arc<ServerInfo>, io: IO) -> Self {
        let cur_servers_task = Self::req_server_list(&io);

        Self {
            server_info,
            browser_data: ServerBrowserData {
                servers: Vec::new(),
                filter: ServerBrowserFilter {
                    exclude: Default::default(),
                    search: Default::default(),
                },
                cur_address: Default::default(),
            },
            menu_io: MainMenuIO {
                io,
                cur_servers_task: Some(cur_servers_task),
            },
        }
    }

    pub(crate) fn get_user_data<'a>(&'a mut self, hide_buttons_right: bool) -> UserData<'a> {
        UserData {
            server_info: &self.server_info,
            browser_data: &mut self.browser_data,
            render_options: RenderOptions { hide_buttons_right },

            main_menu: &mut self.menu_io,
        }
    }
}

impl<'a> UIRenderCallbackFunc<()> for MainMenuUI {
    fn has_blur(&self) -> bool {
        true
    }

    fn render_main_frame(
        &mut self,
        ui: &mut egui::Ui,
        pipe: &mut UIPipe<()>,
        ui_state: &mut UIState,
        graphics: &mut Graphics,
    ) {
        main_frame::render(
            ui,
            &mut UIPipe {
                config: pipe.config,
                cur_time: pipe.cur_time,
                ui_feedback: pipe.ui_feedback,
                user_data: self.get_user_data(false),
            },
            ui_state,
            graphics,
            true,
        );
    }

    fn render(
        &mut self,
        ui: &mut egui::Ui,
        pipe: &mut UIPipe<()>,
        ui_state: &mut UIState,
        graphics: &mut Graphics,
    ) {
        if let Some(server_task) = &self.menu_io.cur_servers_task {
            if server_task.is_finished() {
                let servers_raw = self
                    .menu_io
                    .cur_servers_task
                    .take()
                    .unwrap()
                    .get_storage()
                    .unwrap();
                let servers: BrowserServers = serde_json::from_str(&servers_raw).unwrap();

                let parsed_servers: Vec<ServerBrowserServer> = servers
                    .servers
                    .into_iter()
                    .filter_map(|server| {
                        if let Some(addr) = server
                            .addresses
                            .iter()
                            .find(|addr| addr.protocol == Protocol::V6)
                        {
                            let info: serde_json::Result<ServerBrowserInfo> =
                                serde_json::from_str(server.info.get());
                            match info {
                                Ok(info) => Some(ServerBrowserServer {
                                    address: addr.ip.to_string() + ":" + &addr.port.to_string(),
                                    info,
                                }),
                                Err(err) => {
                                    println!("err {err}");
                                    None
                                }
                            }
                        } else {
                            None
                        }
                    })
                    .collect();

                self.browser_data.servers = parsed_servers;
            }
        }

        main_frame::render(
            ui,
            &mut UIPipe {
                config: pipe.config,
                cur_time: pipe.cur_time,
                ui_feedback: pipe.ui_feedback,
                user_data: self.get_user_data(false),
            },
            ui_state,
            graphics,
            false,
        )
    }
}
