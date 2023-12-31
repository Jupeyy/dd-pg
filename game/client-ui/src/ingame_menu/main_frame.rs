use egui_extras::{Size, StripBuilder};
use graphics::graphics::Graphics;
use ui_base::types::{UIPipe, UIState};

use super::user_data::UserData;

/// top bar
/// big square, rounded edges
pub fn render(
    ui: &mut egui::Ui,
    pipe: &mut UIPipe<UserData>,
    ui_state: &mut UIState,
    graphics: &mut Graphics,
    main_frame_only: bool,
) {
    StripBuilder::new(ui)
        .size(Size::exact(20.0))
        .size(Size::remainder())
        .vertical(|mut strip| {
            strip.cell(|ui| {
                super::topbar::main_frame::render(ui, pipe, ui_state, graphics, main_frame_only);
            });
            strip.cell(|ui| {
                let current_active = pipe
                    .config
                    .ui
                    .path
                    .query
                    .get("game")
                    .map(|s| {
                        if s.is_empty() {
                            "".to_string()
                        } else {
                            s.clone()
                        }
                    })
                    .unwrap_or_default();
                match current_active.as_str() {
                    "Browser" => {
                        crate::main_menu::main_frame::render(
                            ui,
                            &mut UIPipe {
                                ui_feedback: pipe.ui_feedback,
                                cur_time: pipe.cur_time,
                                config: pipe.config,
                                user_data: crate::main_menu::user_data::UserData {
                                    browser_data: pipe.user_data.browser_menu.browser_data,
                                    server_info: pipe.user_data.browser_menu.server_info,
                                    render_options: pipe.user_data.browser_menu.render_options,
                                    main_menu: pipe.user_data.browser_menu.main_menu,
                                },
                            },
                            ui_state,
                            graphics,
                            main_frame_only,
                        );
                    }
                    // "Game"
                    _ => {
                        let dummies_connected = pipe.ui_feedback.local_player_count();
                        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                            ui_state.is_ui_open = false;
                        }
                        ui.horizontal(|ui| {
                            if ui.button("disconnect").clicked() {
                                pipe.ui_feedback.network_disconnect();
                                pipe.ui_feedback.call_path(pipe.config, "", "");
                            }
                            if ui
                                .button(&format!(
                                    "connect dummy ({})",
                                    dummies_connected.max(1) - 1
                                ))
                                .clicked()
                            {
                                pipe.ui_feedback.network_connect_local_player();
                            }
                            if ui.button("disconnect dummy").clicked() {
                                pipe.ui_feedback.network_disconnect_local_player();
                            }
                        });
                    }
                }
            });
        });
}
