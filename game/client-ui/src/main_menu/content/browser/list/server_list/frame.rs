use client_types::server_browser::{ServerBrowserInfo, ServerBrowserServer};
use egui_extras::TableBody;

use ui_base::types::{UiRenderPipe, UiState};

use crate::main_menu::user_data::UserData;

/// server list frame (scrollable)
pub fn render(
    body: TableBody<'_>,
    pipe: &mut UiRenderPipe<UserData>,
    ui_state: &mut UiState,
    cur_page: &str,
) {
    let servers_filtered: Vec<_> = pipe.user_data.browser_data.servers_filtered().collect();
    struct LanServer {
        server: ServerBrowserServer,
        rcon_secret: Option<[u8; 32]>,
    }
    let server_info = &pipe.user_data.server_info;
    let lan_server = [LanServer {
        server: ServerBrowserServer {
            info: ServerBrowserInfo {
                name: "Internal Server".into(),
                version: Default::default(),
                game_type: Default::default(),
                map: Default::default(),
                players: Default::default(),
                passworded: false,
            },
            address: "127.0.0.1:".to_string()
                + &server_info
                    .sock_addr
                    .lock()
                    .unwrap()
                    .map(|addr| addr.port())
                    .unwrap_or_default()
                    .to_string(),
        },
        rcon_secret: *server_info.rcon_secret.lock().unwrap(),
    }];
    body.rows(
        25.0,
        if cur_page != "LAN" {
            servers_filtered.len()
        } else {
            lan_server.len()
        },
        |mut row| {
            let row_index = row.index();
            row.set_selected(
                pipe.user_data
                    .selected_index
                    .is_some_and(|index| index == row_index),
            );
            let clicked = if cur_page != "LAN" {
                super::entry::render(row, servers_filtered[row_index], ui_state)
            } else {
                super::entry::render(row, &lan_server[row_index].server, ui_state)
            };
            if clicked || (cur_page == "LAN" && lan_server.len() == 1) {
                *pipe.user_data.selected_index = Some(row_index);
                pipe.user_data.config.engine.ui.storage.insert(
                    "server-addr".to_string(),
                    if cur_page != "LAN" {
                        servers_filtered[row_index].address.clone()
                    } else {
                        lan_server[row_index].server.address.clone()
                    },
                );

                if cur_page == "LAN" {
                    pipe.user_data.config.engine.ui.storage.insert(
                        "rcon-secret".to_string(),
                        serde_json::to_string(&lan_server[row_index].rcon_secret).unwrap(),
                    );
                }
            }
        },
    );
}
