use egui_extras::{Size, StripBuilder};

use ui_base::types::{UiRenderPipe, UiState};

use crate::{
    events::UiEvent,
    main_menu::constants::{MENU_PROFILE_NAME, MENU_SETTINGS_NAME},
};

use super::{constants::INGAME_MENU_UI_PAGE_QUERY, user_data::UserData};

/// top bar
/// big square, rounded edges
pub fn render(
    ui: &mut egui::Ui,
    pipe: &mut UiRenderPipe<UserData>,
    ui_state: &mut UiState,
    main_frame_only: bool,
) {
    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        ui_state.is_ui_open = false;
    }
    StripBuilder::new(ui)
        .size(Size::exact(20.0))
        .size(Size::remainder())
        .vertical(|mut strip| {
            strip.cell(|ui| {
                super::topbar::main_frame::render(ui, pipe, ui_state, main_frame_only);
            });
            strip.cell(|ui| {
                let current_active = pipe
                    .user_data
                    .browser_menu
                    .config
                    .engine
                    .ui
                    .path
                    .query
                    .get(INGAME_MENU_UI_PAGE_QUERY)
                    .map(|s| {
                        if s.is_empty() {
                            "".to_string()
                        } else {
                            s.clone()
                        }
                    })
                    .unwrap_or_default();
                match current_active.as_str() {
                    "Browser" | MENU_SETTINGS_NAME | MENU_PROFILE_NAME => {
                        crate::main_menu::main_frame::render(
                            ui,
                            &mut UiRenderPipe {
                                cur_time: pipe.cur_time,
                                user_data: &mut crate::main_menu::user_data::UserData {
                                    browser_data: pipe.user_data.browser_menu.browser_data,
                                    server_info: pipe.user_data.browser_menu.server_info,
                                    render_options: pipe.user_data.browser_menu.render_options,
                                    main_menu: pipe.user_data.browser_menu.main_menu,
                                    config: pipe.user_data.browser_menu.config,
                                    events: pipe.user_data.browser_menu.events,
                                    client_info: pipe.user_data.browser_menu.client_info,
                                    selected_index: pipe.user_data.browser_menu.selected_index,
                                    canvas_handle: pipe.user_data.browser_menu.canvas_handle,
                                    render_tee: pipe.user_data.browser_menu.render_tee,
                                    skin_container: pipe.user_data.browser_menu.skin_container,
                                    full_rect: pipe.user_data.browser_menu.full_rect,
                                    profiles: pipe.user_data.browser_menu.profiles,
                                    profile_tasks: pipe.user_data.browser_menu.profile_tasks,
                                    io: pipe.user_data.browser_menu.io,
                                    monitors: pipe.user_data.browser_menu.monitors,
                                },
                            },
                            ui_state,
                            main_frame_only,
                        );
                    }
                    // "Game"
                    _ => {
                        if main_frame_only {
                            return;
                        }
                        let dummies_connected =
                            pipe.user_data.browser_menu.client_info.local_player_count();
                        ui.horizontal(|ui| {
                            if ui.button("disconnect").clicked() {
                                pipe.user_data.browser_menu.events.push(UiEvent::Disconnect);
                                pipe.user_data.browser_menu.config.engine.ui.path.route("");
                            }
                            if ui.button("(dbg) reconnect").clicked() {
                                pipe.user_data.browser_menu.events.push(UiEvent::Disconnect);
                                pipe.user_data.browser_menu.events.push(UiEvent::Connect {
                                    addr: pipe
                                        .user_data
                                        .browser_menu
                                        .config
                                        .engine
                                        .ui
                                        .storage
                                        .get("server-addr")
                                        .cloned()
                                        .unwrap_or_default()
                                        .parse()
                                        .unwrap(),
                                });
                            }
                            if ui
                                .button(&format!(
                                    "connect dummy ({})",
                                    dummies_connected.max(1) - 1
                                ))
                                .clicked()
                            {
                                pipe.user_data
                                    .browser_menu
                                    .events
                                    .push(UiEvent::ConnectLocalPlayer { as_dummy: true });
                            }
                            if ui.button("disconnect dummy").clicked() {
                                pipe.user_data
                                    .browser_menu
                                    .events
                                    .push(UiEvent::DisconnectLocalPlayer);
                            }
                        });
                    }
                }
            });
        });
}
