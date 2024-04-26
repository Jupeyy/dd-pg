use client_types::server_browser::{ServerBrowserInfo, ServerBrowserServer};
use egui_extras::TableBody;

use ui_base::types::{UIPipe, UIState};

use crate::main_menu::user_data::UserData;

/// server list frame (scrollable)
pub fn render(
    body: TableBody<'_>,
    pipe: &mut UIPipe<UserData>,
    ui_state: &mut UIState,
    cur_page: &String,
) {
    let servers_filtered = pipe.user_data.browser_data.servers.iter().filter(|server| {
        server
            .info
            .map
            .to_lowercase()
            .contains(&pipe.user_data.browser_data.filter.search.to_lowercase())
            || server
                .info
                .name
                .to_lowercase()
                .contains(&pipe.user_data.browser_data.filter.search.to_lowercase())
    });
    let lan_server = [ServerBrowserServer {
        info: ServerBrowserInfo {
            name: "Internal Server".into(),
            game_type: Default::default(),
            map: Default::default(),
            map_sha256: Default::default(),
            players: Default::default(),
        },
        address: "127.0.0.1:".to_string()
            + &pipe
                .user_data
                .server_info
                .sock_addr
                .lock()
                .unwrap()
                .map(|addr| addr.port())
                .unwrap_or_default()
                .to_string(),
    }];
    body.rows(
        25.0,
        if cur_page != "LAN" {
            servers_filtered.clone().count()
        } else {
            lan_server.len()
        },
        |row| {
            let row_index = row.index();
            let clicked = if cur_page != "LAN" {
                super::entry::render(row, row_index, servers_filtered.clone(), ui_state)
            } else {
                super::entry::render(row, row_index, lan_server.iter(), ui_state)
            };
            if clicked || cur_page == "LAN" {
                pipe.user_data.config.engine.ui.storage.insert(
                    "server-addr".to_string(),
                    if cur_page != "LAN" {
                        servers_filtered
                            .clone()
                            .nth(row_index)
                            .unwrap()
                            .address
                            .clone()
                    } else {
                        lan_server.iter().nth(row_index).unwrap().address.clone()
                    },
                );
            }
        },
    );
}
