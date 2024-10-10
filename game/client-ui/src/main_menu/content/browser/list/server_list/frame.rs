use std::net::SocketAddr;

use egui_extras::TableBody;
use game_config::config::Config;
use shared_base::server_browser::{ServerBrowserInfo, ServerBrowserServer};

use ui_base::types::UiRenderPipe;

use crate::{
    main_menu::{ddnet_info::DdnetInfo, favorite_player::FavoritePlayers, user_data::UserData},
    sort::{SortDir, TableSort},
};

fn servers_filtered<'a>(
    servers: &'a [ServerBrowserServer],
    config: &Config,
    favorites: &'a FavoritePlayers,
    ddnet_info: &'a DdnetInfo,
) -> impl Iterator<Item = &'a ServerBrowserServer> {
    let search = config.storage::<String>("filter.search");
    let has_players = config.storage::<bool>("filter.has_players");
    let server_full = config.storage::<bool>("filter.server_full");
    let fav_players_only = config.storage::<bool>("filter.fav_players_only");
    let no_password = config.storage::<bool>("filter.no_password");
    let unfinished_maps = config.storage::<bool>("filter.unfinished_maps");
    servers.iter().filter(move |server| {
        (server
            .info
            .map
            .name
            .to_lowercase()
            .contains(&search.to_lowercase())
            || server
                .info
                .name
                .to_lowercase()
                .contains(&search.to_lowercase()))
            && (!has_players || !server.info.players.is_empty())
            && (!server_full || server.info.players.len() < server.info.max_players as usize)
            && (!no_password || !server.info.passworded)
            && (!fav_players_only
                || server
                    .info
                    .players
                    .iter()
                    .any(|p| favorites.iter().any(|f| f.name == p.name)))
            && (!unfinished_maps || ddnet_info.maps.contains(&server.info.map.name))
    })
}

fn servers_sorted(servers: &mut [&ServerBrowserServer], config: &Config) {
    let sort: TableSort = config.storage("filter.sort");
    servers.sort_by(|d1, d2| {
        let order = match sort.name.as_str() {
            "Name" => d1.info.name.cmp(&d2.info.name),
            "Type" => d1.info.game_type.cmp(&d2.info.game_type),
            "Map" => d1.info.map.name.cmp(&d2.info.map.name),
            "Players" => d1.info.players.len().cmp(&d2.info.players.len()),
            // TODO: "Ping"
            _ => d1.info.name.cmp(&d2.info.name),
        };

        match sort.sort_dir {
            SortDir::Asc => order,
            SortDir::Desc => order.reverse(),
        }
    });
}

/// server list frame (scrollable)
pub fn render(mut body: TableBody<'_>, pipe: &mut UiRenderPipe<UserData>, cur_page: &str) {
    let ddnet_info = &pipe.user_data.ddnet_info;
    let favorites = pipe
        .user_data
        .config
        .storage::<FavoritePlayers>("favorite-players");
    let mut servers_filtered: Vec<_> = servers_filtered(
        &pipe.user_data.browser_data.servers,
        pipe.user_data.config,
        &favorites,
        ddnet_info,
    )
    .collect();
    servers_sorted(&mut servers_filtered, pipe.user_data.config);
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
                max_players: u32::MAX,
                passworded: false,
                cert_sha256_fingerprint: Default::default(),
            },
            address: "127.0.0.1:".to_string()
                + &server_info
                    .sock_addr
                    .lock()
                    .unwrap()
                    .map(|addr| addr.port())
                    .unwrap_or_default()
                    .to_string(),
            location: "default".to_string(),
        },
        rcon_secret: *server_info.rcon_secret.lock().unwrap(),
    }];

    let select_prev = body
        .ui_mut()
        .ctx()
        .input(|i| i.key_pressed(egui::Key::ArrowUp))
        && body.ui_mut().ctx().memory(|m| m.focused().is_none());
    let select_next = body
        .ui_mut()
        .ctx()
        .input(|i| i.key_pressed(egui::Key::ArrowDown))
        && body.ui_mut().ctx().memory(|m| m.focused().is_none());
    let select_first = body
        .ui_mut()
        .ctx()
        .input(|i| i.key_pressed(egui::Key::PageUp))
        && body.ui_mut().ctx().memory(|m| m.focused().is_none());
    let select_last = body
        .ui_mut()
        .ctx()
        .input(|i| i.key_pressed(egui::Key::PageDown))
        && body.ui_mut().ctx().memory(|m| m.focused().is_none());

    let cur_addr: String = pipe
        .user_data
        .config
        .storage_opt::<SocketAddr>("server-addr")
        .map(|a| a.to_string())
        .unwrap_or_default();

    body.rows(
        30.0,
        if cur_page != "LAN" {
            servers_filtered.len()
        } else {
            lan_server.len()
        },
        |mut row| {
            let row_index = row.index();

            let server = if cur_page != "LAN" {
                servers_filtered[row_index]
            } else {
                &lan_server[row_index].server
            };

            let select_index = if select_prev {
                Some(row_index + 1)
            } else if select_next {
                Some(row_index.saturating_sub(1))
            } else if select_first {
                Some(0)
            } else if select_last {
                Some(if cur_page != "LAN" {
                    servers_filtered.len().saturating_sub(1)
                } else {
                    lan_server.len().saturating_sub(1)
                })
            } else {
                None
            };

            let is_selected = server.address == cur_addr;
            row.set_selected(is_selected);
            let clicked = super::entry::render(row, server)
                || (cur_page == "LAN" && lan_server.len() == 1)
                || select_index
                    .and_then(|index| {
                        if cur_page != "LAN" {
                            servers_filtered.get(index).copied()
                        } else {
                            lan_server.get(index).map(|s| &s.server)
                        }
                    })
                    .is_some_and(|s| s.address == cur_addr);

            if clicked || is_selected {
                if let Ok(addr) = server.address.parse::<SocketAddr>() {
                    // extra check here, bcs the server addr might be changed by keyboard
                    if clicked {
                        pipe.user_data.config.set_storage("server-addr", &addr);
                    }
                    pipe.user_data.config.set_storage(
                        "server-cert",
                        &if cur_page != "LAN" {
                            Some(server.info.cert_sha256_fingerprint)
                        } else {
                            None
                        },
                    );
                    if cur_page == "LAN" {
                        pipe.user_data.config.rem_storage("server-cert");

                        pipe.user_data
                            .config
                            .set_storage("rcon-secret", &lan_server[row_index].rcon_secret);
                    }
                }
            }
        },
    );
}
