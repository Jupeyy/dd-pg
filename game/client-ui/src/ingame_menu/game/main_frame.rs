use egui::{Color32, DragValue, Layout, Rounding, Window};
use egui_extras::{Size, StripBuilder};
use game_config::config::ConfigDummyScreenAnchor;
use ui_base::{
    types::{UiRenderPipe, UiState},
    utils::icon_font_text,
};

use crate::{events::UiEvent, ingame_menu::user_data::UserData};

pub fn render(
    ui: &mut egui::Ui,
    pipe: &mut UiRenderPipe<UserData>,
    _ui_state: &mut UiState,
    main_frame_only: bool,
) {
    let mut frame_rect = ui.available_rect_before_wrap();
    frame_rect.set_height(20.0);
    if main_frame_only {
        ui.painter()
            .rect_filled(frame_rect, Rounding::default(), Color32::BLACK);
        return;
    } else {
        ui.painter().rect_filled(
            frame_rect,
            Rounding::default(),
            Color32::from_black_alpha(50),
        );
    }
    let players_connected = pipe.user_data.browser_menu.client_info.local_player_count();
    StripBuilder::new(ui)
        .size(Size::relative(0.3))
        .size(Size::remainder())
        .horizontal(|mut strip| {
            strip.cell(|ui| {
                ui.horizontal(|ui| {
                    ui.button("Spectate");
                    if ui.button("Kill").clicked() {
                        pipe.user_data.browser_menu.events.push(UiEvent::Kill);
                    }
                    ui.button("Pause");
                });
            });
            strip.cell(|ui| {
                ui.with_layout(
                    Layout::right_to_left(egui::Align::Min).with_main_wrap(true),
                    |ui| {
                        if ui.button("Disconnect").clicked() {
                            pipe.user_data.browser_menu.events.push(UiEvent::Disconnect);
                            pipe.user_data.browser_menu.config.engine.ui.path.route("");
                        }
                        if pipe.user_data.browser_menu.config.engine.dbg.app {
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
                                    cert_hash: None,

                                    rcon_secret: pipe
                                        .user_data
                                        .browser_menu
                                        .config
                                        .engine
                                        .ui
                                        .storage
                                        .get("rcon-secret")
                                        .and_then(|rcon_secret| {
                                            serde_json::from_str::<Option<[u8; 32]>>(rcon_secret)
                                                .ok()
                                        })
                                        .unwrap_or_default(),
                                });
                            }
                            if ui
                                .button(format!(
                                    "(dbg) connect dummy ({})",
                                    players_connected.saturating_sub(1)
                                ))
                                .clicked()
                            {
                                pipe.user_data
                                    .browser_menu
                                    .events
                                    .push(UiEvent::ConnectLocalPlayer { as_dummy: true });
                            }
                            if ui.button("(dbg) disconnect dummy").clicked() {
                                pipe.user_data
                                    .browser_menu
                                    .events
                                    .push(UiEvent::DisconnectLocalPlayer);
                            }
                        } else {
                            ui.horizontal(|ui| {
                                let game_config = &mut pipe.user_data.browser_menu.config.game;
                                let path = &mut pipe.user_data.browser_menu.config.engine.ui.path;
                                ui.style_mut().spacing.item_spacing.x = 0.0;

                                ui.menu_button(icon_font_text(ui, "\u{f013}"), |ui| {
                                    if ui.button("Dummy settings").clicked() {
                                        // settings like if a mini screen of the dummy should show up
                                        // and how big this screen should be etc.
                                        path.route_query_only_single((
                                            "dummy_settings".to_string(),
                                            "1".to_string(),
                                        ));
                                    }
                                });

                                // dummy settings
                                let show_dummy_settings = path
                                    .query
                                    .get("dummy_settings")
                                    .map(|v| v == "1")
                                    .unwrap_or_default();
                                if show_dummy_settings {
                                    let mut open = show_dummy_settings;
                                    Window::new("Dummy settings")
                                        .open(&mut open)
                                        .collapsible(false)
                                        .show(ui.ctx(), |ui| {
                                            let dummy = &mut game_config.cl.dummy;
                                            ui.checkbox(
                                                &mut dummy.mini_screen,
                                                "Show dummy in mini screen.",
                                            );
                                            ui.label("Sceen width:");
                                            ui.add(
                                                DragValue::new(&mut dummy.screen_width)
                                                    .clamp_range(1..=100),
                                            );
                                            ui.label("Sceen height:");
                                            ui.add(
                                                DragValue::new(&mut dummy.screen_height)
                                                    .clamp_range(1..=100),
                                            );

                                            let anchors = [
                                                "Top left",
                                                "Top right",
                                                "Bottom left",
                                                "Bottom right",
                                            ];
                                            egui::ComboBox::new("select-anchor", "")
                                                .selected_text(
                                                    anchors[dummy.screen_anchor as usize],
                                                )
                                                .show_ui(ui, |ui| {
                                                    let mut btn =
                                                        |anchor: ConfigDummyScreenAnchor| {
                                                            if ui
                                                                .button(anchors[anchor as usize])
                                                                .clicked()
                                                            {
                                                                dummy.screen_anchor = anchor;
                                                            }
                                                        };
                                                    btn(ConfigDummyScreenAnchor::TopLeft);
                                                    btn(ConfigDummyScreenAnchor::TopRight);
                                                    btn(ConfigDummyScreenAnchor::BottomLeft);
                                                    btn(ConfigDummyScreenAnchor::BottomRight);
                                                });
                                        });
                                    if !open {
                                        path.query.remove("dummy_settings");
                                    }
                                }

                                if players_connected > 1 {
                                    if ui.button("Disconnect dummy").clicked() {
                                        pipe.user_data
                                            .browser_menu
                                            .events
                                            .push(UiEvent::DisconnectLocalPlayer);
                                    }
                                } else if ui.button("Connect dummy").clicked() {
                                    pipe.user_data
                                        .browser_menu
                                        .events
                                        .push(UiEvent::ConnectLocalPlayer { as_dummy: true });
                                }
                            });
                        }

                        if ui.button("Record demo").clicked() {
                            pipe.user_data.browser_menu.events.push(UiEvent::RecordDemo);
                        }
                    },
                );
            });
        });
}
