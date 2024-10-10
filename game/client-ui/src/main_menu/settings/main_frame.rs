use egui::{epaint::RectShape, Button, Color32, Frame, Layout, Rect, Rounding, Shape};
use egui_extras::{Size, StripBuilder};

use ui_base::{
    types::{UiRenderPipe, UiState},
    utils::add_horizontal_margins,
};

use crate::main_menu::{constants::MENU_SETTINGS_NAME, user_data::UserData};

use super::constants::{SETTINGS_SUB_UI_PAGE_QUERY, SETTINGS_UI_PAGE_QUERY};

fn render_nav(
    ui: &mut egui::Ui,
    pipe: &mut UiRenderPipe<UserData>,
    cur_sub: &str,
    cur_subsub: &str,
) {
    ui.style_mut().spacing.item_spacing.x = 0.0;
    ui.horizontal(|ui| {
        ui.add_space(10.0);
        ui.with_layout(
            Layout::top_down(egui::Align::Min).with_cross_justify(true),
            |ui| {
                let mut add_btn = |ui: &mut egui::Ui, s: &str, submenu: Option<&str>| {
                    let selected = (submenu.is_none() && cur_sub == s && cur_subsub.is_empty())
                        || (submenu.is_some() && cur_subsub == s);
                    let bg_idx = ui.painter().add(Shape::Noop);
                    let bgsub_idx = ui.painter().add(Shape::Noop);
                    let btn = ui.add(Button::new(s).frame(false));
                    if btn.clicked() {
                        let path = &mut pipe.user_data.config.engine.ui.path;
                        if let Some(parent) = submenu {
                            path.route_query_only_single((
                                SETTINGS_UI_PAGE_QUERY.to_string(),
                                parent.to_string(),
                            ));
                            path.route_query_only_single((
                                SETTINGS_SUB_UI_PAGE_QUERY.to_string(),
                                s.to_string(),
                            ));
                        } else {
                            path.route_query_only_single((
                                SETTINGS_UI_PAGE_QUERY.to_string(),
                                s.to_string(),
                            ));
                            path.route_query_only_single((
                                SETTINGS_SUB_UI_PAGE_QUERY.to_string(),
                                "".to_string(),
                            ));
                        }
                    }

                    if submenu.is_some() {
                        let btn_rect = btn
                            .rect
                            .expand2(egui::vec2(6.0, 0.0))
                            .translate(egui::vec2(-6.0, 0.0));

                        ui.painter().set(
                            bgsub_idx,
                            Shape::rect_filled(
                                btn_rect,
                                Rounding::default(),
                                Color32::from_black_alpha(50),
                            ),
                        );
                    }

                    if selected {
                        let mut offset = btn.rect.left_center();
                        if submenu.is_some() {
                            offset.x -= 2.0;
                        }
                        ui.painter().rect_filled(
                            Rect::from_center_size(offset, egui::vec2(4.0, btn.rect.height()))
                                .translate(egui::vec2(-8.0, 0.0)),
                            Rounding::default(),
                            Color32::LIGHT_BLUE,
                        );
                        let mut btn_rect = btn
                            .rect
                            .expand2(egui::vec2(3.0, 0.0))
                            .translate(egui::vec2(-3.0, 0.0));

                        if submenu.is_some() {
                            btn_rect = btn_rect
                                .expand2(egui::vec2(2.0, 0.0))
                                .translate(egui::vec2(-2.0, 0.0));
                        }
                        ui.painter().set(
                            bg_idx,
                            Shape::rect_filled(
                                btn_rect,
                                Rounding::default(),
                                Color32::from_rgba_unmultiplied(
                                    Color32::LIGHT_BLUE.r(),
                                    Color32::LIGHT_BLUE.g(),
                                    Color32::LIGHT_BLUE.b(),
                                    5,
                                ),
                            ),
                        );
                    }
                };

                ui.add_space(10.0);
                add_btn(ui, "General", None);
                add_btn(ui, "Language", None);
                ui.add_space(10.0);

                let old_spacing_y =
                    std::mem::replace(&mut ui.style_mut().spacing.item_spacing.y, 0.0);
                add_btn(ui, "Player", None);
                ui.horizontal(|ui| {
                    ui.add_space(8.0);
                    ui.with_layout(
                        Layout::top_down(egui::Align::Min).with_cross_justify(true),
                        |ui| {
                            add_btn(ui, "Tee", Some("Player"));
                            add_btn(ui, "Misc", Some("Player"));
                            add_btn(ui, "Assets", Some("Player"));
                            add_btn(ui, "Controls", Some("Player"));
                        },
                    );
                });
                ui.style_mut().spacing.item_spacing.y = old_spacing_y;

                ui.add_space(10.0);
                add_btn(ui, "Graphics", None);

                let old_spacing_y =
                    std::mem::replace(&mut ui.style_mut().spacing.item_spacing.y, 0.0);
                add_btn(ui, "Sound", None);
                ui.horizontal(|ui| {
                    ui.add_space(8.0);
                    ui.with_layout(
                        Layout::top_down(egui::Align::Min).with_cross_justify(true),
                        |ui| {
                            add_btn(ui, "Spatial Chat", Some("Sound"));
                        },
                    );
                });
                ui.style_mut().spacing.item_spacing.y = old_spacing_y;
            },
        );
    });
}

/// big box, rounded edges
pub fn render(
    ui: &mut egui::Ui,
    pipe: &mut UiRenderPipe<UserData>,
    ui_state: &mut UiState,
    cur_page: &str,
    main_frame_only: bool,
) {
    if cur_page == MENU_SETTINGS_NAME {
        let path = &mut pipe.user_data.config.engine.ui.path;
        let cur_sub = path
            .query
            .get(SETTINGS_UI_PAGE_QUERY)
            .map(|path| path.as_ref())
            .unwrap_or("")
            .to_string();

        let cur_subsub = path
            .query
            .get(SETTINGS_SUB_UI_PAGE_QUERY)
            .map(|path| path.as_ref())
            .unwrap_or("")
            .to_string();

        let width_nav = 80.0;
        StripBuilder::new(ui)
            .size(Size::remainder())
            .size(Size::exact(width_nav))
            .horizontal(|mut strip| {
                strip.cell(|ui| {
                    if main_frame_only {
                        ui.painter().add(Shape::Rect(RectShape::filled(
                            ui.available_rect_before_wrap(),
                            5.0,
                            Color32::from_rgba_unmultiplied(0, 0, 0, 255),
                        )));
                    } else {
                        Frame::default()
                            .fill(Color32::from_rgba_unmultiplied(0, 0, 0, 100))
                            .rounding(5.0)
                            .show(ui, |ui| {
                                add_horizontal_margins(ui, |ui| {
                                    StripBuilder::new(ui)
                                        .size(Size::exact(0.0))
                                        .size(Size::remainder())
                                        .size(Size::exact(0.0))
                                        .clip(true)
                                        .vertical(|mut strip| {
                                            strip.empty();
                                            strip.cell(|ui| {
                                                match cur_sub.as_str() {
                                                    "Player" => {
                                                        super::player::main_frame::render(
                                                            ui, pipe, ui_state,
                                                        );
                                                    }
                                                    "Graphics" => {
                                                        super::graphics::main_frame::render(
                                                            ui, pipe,
                                                        );
                                                    }
                                                    "Sound" => {
                                                        super::sound::main_frame::render(ui, pipe);
                                                    }
                                                    // general is default
                                                    _ => {
                                                        super::general::main_frame::render(
                                                            ui, pipe, ui_state,
                                                        );
                                                    }
                                                }
                                            });
                                            strip.empty();
                                        });
                                });
                            });
                    }
                });
                strip.cell(|ui| {
                    if main_frame_only {
                        ui.painter().add(Shape::Rect(RectShape::filled(
                            ui.available_rect_before_wrap(),
                            5.0,
                            Color32::from_rgba_unmultiplied(0, 0, 0, 255),
                        )));
                    } else {
                        Frame::default()
                            .fill(Color32::from_rgba_unmultiplied(0, 0, 0, 100))
                            .rounding(5.0)
                            .show(ui, |ui| {
                                StripBuilder::new(ui)
                                    .size(Size::exact(0.0))
                                    .size(Size::remainder())
                                    .size(Size::exact(0.0))
                                    .clip(true)
                                    .vertical(|mut strip| {
                                        strip.empty();
                                        strip.cell(|ui| {
                                            render_nav(ui, pipe, &cur_sub, &cur_subsub);
                                        });
                                        strip.empty();
                                    });
                            });
                    }
                });
            });
    }
}
