use egui_extras::{Size, StripBuilder};

use ui_base::types::{UiRenderPipe, UiState};

use crate::main_menu::constants::{MENU_DEMO_NAME, MENU_PROFILE_NAME, MENU_SETTINGS_NAME};

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
                super::topbar::main_frame::render(ui, pipe, main_frame_only);
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
                    "Browser" | MENU_SETTINGS_NAME | MENU_PROFILE_NAME | MENU_DEMO_NAME => {
                        crate::main_menu::main_frame::render(
                            ui,
                            &mut UiRenderPipe {
                                cur_time: pipe.cur_time,
                                user_data: &mut crate::main_menu::user_data::UserData {
                                    browser_data: pipe.user_data.browser_menu.browser_data,
                                    ddnet_info: pipe.user_data.browser_menu.ddnet_info,
                                    icons: pipe.user_data.browser_menu.icons,

                                    demos: pipe.user_data.browser_menu.demos,
                                    demo_info: pipe.user_data.browser_menu.demo_info,
                                    server_info: pipe.user_data.browser_menu.server_info,
                                    render_options: pipe.user_data.browser_menu.render_options,
                                    main_menu: pipe.user_data.browser_menu.main_menu,
                                    config: pipe.user_data.browser_menu.config,
                                    events: pipe.user_data.browser_menu.events,
                                    client_info: pipe.user_data.browser_menu.client_info,

                                    graphics_mt: pipe.user_data.browser_menu.graphics_mt,
                                    backend_handle: pipe.user_data.browser_menu.backend_handle,
                                    stream_handle: pipe.user_data.browser_menu.stream_handle,
                                    canvas_handle: pipe.user_data.browser_menu.canvas_handle,
                                    texture_handle: pipe.user_data.browser_menu.texture_handle,

                                    render_tee: pipe.user_data.browser_menu.render_tee,
                                    flags_container: pipe.user_data.browser_menu.flags_container,
                                    skin_container: pipe.user_data.browser_menu.skin_container,
                                    toolkit_render: pipe.user_data.browser_menu.toolkit_render,
                                    weapons_container: pipe
                                        .user_data
                                        .browser_menu
                                        .weapons_container,
                                    hook_container: pipe.user_data.browser_menu.hook_container,
                                    entities_container: pipe
                                        .user_data
                                        .browser_menu
                                        .entities_container,
                                    freeze_container: pipe.user_data.browser_menu.freeze_container,
                                    emoticons_container: pipe
                                        .user_data
                                        .browser_menu
                                        .emoticons_container,
                                    particles_container: pipe
                                        .user_data
                                        .browser_menu
                                        .particles_container,
                                    ninja_container: pipe.user_data.browser_menu.ninja_container,
                                    game_container: pipe.user_data.browser_menu.game_container,
                                    hud_container: pipe.user_data.browser_menu.hud_container,
                                    ctf_container: pipe.user_data.browser_menu.ctf_container,
                                    theme_container: pipe.user_data.browser_menu.theme_container,

                                    map_render: pipe.user_data.browser_menu.map_render,
                                    tile_set_preview: pipe.user_data.browser_menu.tile_set_preview,

                                    spatial_chat: pipe.user_data.browser_menu.spatial_chat,
                                    player_settings_sync: pipe
                                        .user_data
                                        .browser_menu
                                        .player_settings_sync,

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
                    "Players" => super::server_players::main_frame::render(ui, pipe),
                    "Server info" => {
                        let game_info = pipe.user_data.game_server_info.game_info();
                        ui.label(format!("Map: {}", game_info.map_name));
                    }
                    "Call vote" => super::call_vote::main_frame::render(ui, pipe),
                    "Account" => super::account::main_frame::render(ui, pipe, main_frame_only),
                    // "Game"
                    _ => super::game::main_frame::render(ui, pipe, main_frame_only),
                }
            });
        });
}
