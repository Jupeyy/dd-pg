use std::net::SocketAddr;

use config::types::ConfRgb;
use egui::{Color32, DragValue, Layout, Rounding, TextEdit, Window};
use egui_extras::{Size, StripBuilder};
use game_config::config::ConfigDummyScreenAnchor;
use math::math::vector::ubvec4;
use ui_base::{types::UiRenderPipe, utils::icon_font_text_for_btn};

use crate::{events::UiEvent, ingame_menu::user_data::UserData};

pub fn render(ui: &mut egui::Ui, pipe: &mut UiRenderPipe<UserData>, main_frame_only: bool) {
    let config = &mut pipe.user_data.browser_menu.config;
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
                    if ui.button("Spectate").clicked() {
                        pipe.user_data
                            .browser_menu
                            .events
                            .push(UiEvent::JoinSpectators);
                    }
                    if ui.button("Kill").clicked() {
                        pipe.user_data.browser_menu.events.push(UiEvent::Kill);
                    }
                    if ui.button("Pause").clicked() {
                        pipe.user_data
                            .browser_menu
                            .events
                            .push(UiEvent::SwitchToFreeCam);
                    }

                    if pipe
                        .user_data
                        .game_server_info
                        .server_options()
                        .allow_stages
                    {
                        ui.horizontal(|ui| {
                            ui.style_mut().spacing.item_spacing.x = 0.0;

                            ui.menu_button("Team", |ui| {
                                let team = &mut config.game.cl.team;

                                if ui.button("Join your team").clicked() {
                                    pipe.user_data
                                        .browser_menu
                                        .events
                                        .push(UiEvent::JoinOwnTeam {
                                            name: team.name.to_string(),
                                            color: ubvec4::new(
                                                team.color.r,
                                                team.color.g,
                                                team.color.b,
                                                255,
                                            ),
                                        });
                                }
                                if ui.button("Join other team").clicked() {
                                    pipe.user_data
                                        .browser_menu
                                        .events
                                        .push(UiEvent::JoinOtherTeam);
                                }
                            });

                            if ui.button(icon_font_text_for_btn(ui, "\u{f013}")).clicked() {
                                // Settings like team color and name
                                config.path().route_query_only_single((
                                    "team_settings".to_string(),
                                    "1".to_string(),
                                ));
                            }
                        });
                    }

                    if pipe
                        .user_data
                        .game_server_info
                        .server_options()
                        .use_vanilla_sides
                    {
                        ui.menu_button("Pick side", |ui| {
                            if ui.button("Red side").clicked() {
                                pipe.user_data
                                    .browser_menu
                                    .events
                                    .push(UiEvent::JoinVanillaSide { is_red_side: true });
                            }
                            if ui.button("Blue side").clicked() {
                                pipe.user_data
                                    .browser_menu
                                    .events
                                    .push(UiEvent::JoinVanillaSide { is_red_side: false });
                            }
                        });
                    }
                });

                let show_dummy_settings = config
                    .path()
                    .query
                    .get("team_settings")
                    .map(|v| v == "1")
                    .unwrap_or_default();
                if show_dummy_settings {
                    let mut open = show_dummy_settings;
                    Window::new("Team settings")
                        .open(&mut open)
                        .collapsible(false)
                        .show(ui.ctx(), |ui| {
                            let team = &mut config.game.cl.team;

                            ui.label("Name:");
                            if ui
                                .add(TextEdit::singleline(&mut team.name).char_limit(24))
                                .changed()
                            {
                                pipe.user_data
                                    .browser_menu
                                    .player_settings_sync
                                    .set_team_settings_changed();
                            }
                            ui.label("Color:");
                            let mut colors = [
                                team.color.r as f32 / 255.0,
                                team.color.g as f32 / 255.0,
                                team.color.b as f32 / 255.0,
                            ];
                            if ui.color_edit_button_rgb(&mut colors).changed() {
                                pipe.user_data
                                    .browser_menu
                                    .player_settings_sync
                                    .set_team_settings_changed();
                            }
                            team.color = ConfRgb {
                                r: (colors[0] * 255.0) as u8,
                                g: (colors[1] * 255.0) as u8,
                                b: (colors[2] * 255.0) as u8,
                            };
                        });
                    if !open {
                        config.path().query.remove("team_settings");
                    }
                }
            });
            strip.cell(|ui| {
                ui.with_layout(
                    Layout::right_to_left(egui::Align::Min).with_main_wrap(true),
                    |ui| {
                        if ui.button("Disconnect").clicked() {
                            pipe.user_data.browser_menu.events.push(UiEvent::Disconnect);
                            config.path().route("");
                        }
                        if config.engine.dbg.app {
                            if ui.button("(dbg) reconnect").clicked() {
                                pipe.user_data.browser_menu.events.push(UiEvent::Disconnect);
                                pipe.user_data.browser_menu.events.push(UiEvent::Connect {
                                    addr: config.storage_opt("server-addr").unwrap_or_else(|| {
                                        SocketAddr::V4("127.0.0.1".parse().unwrap())
                                    }),
                                    cert_hash: None,

                                    rcon_secret: config.storage("rcon-secret"),
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
                                ui.style_mut().spacing.item_spacing.x = 0.0;

                                ui.menu_button(icon_font_text_for_btn(ui, "\u{f013}"), |ui| {
                                    if ui.button("Dummy settings").clicked() {
                                        // settings like if a mini screen of the dummy should show up
                                        // and how big this screen should be etc.
                                        config.path().route_query_only_single((
                                            "dummy_settings".to_string(),
                                            "1".to_string(),
                                        ));
                                    }
                                });

                                // dummy settings
                                let show_dummy_settings = config
                                    .path()
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
                                            let dummy = &mut config.game.cl.dummy;
                                            ui.checkbox(
                                                &mut dummy.mini_screen,
                                                "Show dummy in mini screen.",
                                            );
                                            ui.label("Sceen width:");
                                            ui.add(
                                                DragValue::new(&mut dummy.screen_width)
                                                    .range(1..=100),
                                            );
                                            ui.label("Sceen height:");
                                            ui.add(
                                                DragValue::new(&mut dummy.screen_height)
                                                    .range(1..=100),
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
                                        config.path().query.remove("dummy_settings");
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
