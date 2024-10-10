use std::collections::HashSet;

use client_containers::container::ContainerItemIndexType;
use egui::{Align, Layout, Rect, UiBuilder};
use egui_extras::{Size, StripBuilder};

use ui_base::{
    types::{UiRenderPipe, UiState},
    utils::icon_font_text_for_btn,
};

use crate::{main_menu::user_data::UserData, utils::render_flag_for_ui};

/// button & popover
pub fn render(ui: &mut egui::Ui, pipe: &mut UiRenderPipe<UserData>, ui_state: &mut UiState) {
    let search_width = if ui.available_width() < 350.0 {
        150.0
    } else {
        250.0
    };
    let extra_space = 0.0;
    StripBuilder::new(ui)
        .size(Size::exact(extra_space))
        .size(Size::exact(30.0))
        .size(Size::remainder().at_least(search_width))
        .size(Size::exact(30.0))
        .size(Size::exact(extra_space))
        .horizontal(|mut strip| {
            strip.empty();
            strip.cell(|ui| {
                ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                    // hamburger menu
                    ui.menu_button(icon_font_text_for_btn(ui, "\u{f0c9}"), |ui| {
                        if ui.button("Save current filter in tab").clicked() {
                            // TODO:
                        }
                    });
                });
            });
            strip.cell(|ui| {
                StripBuilder::new(ui)
                    .size(Size::remainder())
                    .size(Size::exact(search_width))
                    .size(Size::remainder())
                    .horizontal(|mut strip| {
                        strip.empty();
                        strip.cell(|ui| {
                            ui.with_layout(
                                Layout::left_to_right(Align::Center).with_main_justify(true),
                                |ui| {
                                    super::search::render(ui, pipe);
                                },
                            );
                        });
                        strip.empty();
                    });
            });
            strip.cell(|ui| {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    // filter
                    ui.menu_button(icon_font_text_for_btn(ui, "\u{f0b0}"), |ui| {
                        let config = &mut *pipe.user_data.config;
                        // filter window
                        let mut has_players = config.storage::<bool>("filter.has_players");
                        if ui.checkbox(&mut has_players, "Has players").changed() {
                            config.set_storage("filter.has_players", &has_players);
                        }
                        let mut server_full = config.storage::<bool>("filter.server_full");
                        if ui.checkbox(&mut server_full, "Server not full").changed() {
                            config.set_storage("filter.server_full", &server_full);
                        }
                        let mut fav_players_only =
                            config.storage::<bool>("filter.fav_players_only");
                        if ui
                            .checkbox(&mut fav_players_only, "Favorite players only")
                            .changed()
                        {
                            config.set_storage("filter.fav_players_only", &fav_players_only);
                        }
                        let mut no_password = config.storage::<bool>("filter.no_password");
                        if ui.checkbox(&mut no_password, "No password").changed() {
                            config.set_storage("filter.no_password", &no_password);
                        }
                        let mut unfinished_maps = config.storage::<bool>("filter.unfinished_maps");
                        if ui
                            .checkbox(&mut unfinished_maps, "Unfinished maps only")
                            .changed()
                        {
                            config.set_storage("filter.unfinished_maps", &unfinished_maps);
                        }

                        // list countries and mod types
                        let left_top = ui.available_rect_before_wrap().left_top();
                        ui.allocate_new_ui(
                            UiBuilder::new().max_rect(Rect::from_min_max(
                                left_top,
                                left_top + egui::vec2(150.0, 150.0),
                            )),
                            |ui| {
                                let servers = &pipe.user_data.browser_data;
                                let server_locations: HashSet<String> = servers
                                    .servers
                                    .iter()
                                    .map(|s| {
                                        s.location
                                            .to_lowercase()
                                            .split_once(":")
                                            .map(|(s1, s2)| if s2.is_empty() { s1 } else { s2 })
                                            .unwrap_or("default")
                                            .to_string()
                                    })
                                    .collect();
                                super::super::super::settings::list::list::render(
                                    ui,
                                    server_locations
                                        .iter()
                                        .map(|s| (s.as_str(), ContainerItemIndexType::Disk)),
                                    20.0,
                                    |_, _| Ok(()),
                                    |_, _| true,
                                    |ui, _, name, pos, size| {
                                        let key =
                                            pipe.user_data.flags_container.default_key.clone();
                                        render_flag_for_ui(
                                            pipe.user_data.stream_handle,
                                            pipe.user_data.canvas_handle,
                                            pipe.user_data.flags_container,
                                            ui,
                                            ui_state,
                                            ui.ctx().screen_rect(),
                                            Some(ui.available_rect_before_wrap()),
                                            &key,
                                            &name.to_lowercase().replace("-", "_"),
                                            pos,
                                            size,
                                        );
                                    },
                                    |_, _| {},
                                    &mut String::new(),
                                    |_| {},
                                );
                            },
                        );

                        if ui.button("Reset filter").clicked() {
                            config.rem_storage("filter.has_players");
                            config.rem_storage("filter.server_full");
                            config.rem_storage("filter.fav_players_only");
                            config.rem_storage("filter.no_password");
                            config.rem_storage("filter.unfinished_maps");
                        }
                    });
                });
            });
            strip.empty();
        });
}
