use std::sync::Arc;

use client_types::server_browser::ServerBrowserData;
use shared_base::network::server_info::ServerInfo;

#[derive(Debug, Clone, Copy)]
pub struct RenderOptions {
    pub hide_buttons_right: bool,
}

pub trait MainMenuInterface {
    fn refresh(&mut self);
}

pub struct UserData<'a> {
    pub browser_data: &'a mut ServerBrowserData,
    pub server_info: &'a Arc<ServerInfo>,

    pub render_options: RenderOptions,

    pub main_menu: &'a mut dyn MainMenuInterface,
}
