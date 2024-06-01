use egui::{epaint::RectShape, Button, Color32, Frame, Layout, Shape};
use egui_extras::{Size, StripBuilder};

use ui_base::{
    types::{UiRenderPipe, UiState},
    utils::add_horizontal_margins,
};

use crate::main_menu::{constants::MENU_SETTINGS_NAME, user_data::UserData};

use super::constants::SETTINGS_UI_PAGE_QUERY;

/// big box, rounded edges
pub fn render(
    ui: &mut egui::Ui,
    pipe: &mut UiRenderPipe<UserData>,
    ui_state: &mut UiState,
    cur_page: &str,
    main_frame_only: bool,
) {
    if cur_page == MENU_SETTINGS_NAME {
        let cur_sub = pipe
            .user_data
            .config
            .engine
            .ui
            .path
            .query
            .get(SETTINGS_UI_PAGE_QUERY)
            .map(|path| path.as_ref())
            .unwrap_or("")
            .to_string();
        let width_nav = 100.0;
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
                                                    "Tee" => {
                                                        super::tee::main_frame::render(
                                                            ui, pipe, ui_state,
                                                        );
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
                                                ui.with_layout(
                                                    Layout::top_down(egui::Align::Center)
                                                        .with_cross_justify(true),
                                                    |ui| {
                                                        let mut add_btn = |s: &str| {
                                                            if ui
                                                                .add(
                                                                    Button::new(s)
                                                                        .selected(cur_sub == s),
                                                                )
                                                                .clicked()
                                                            {
                                                                pipe.user_data
                                                                    .config
                                                                    .engine
                                                                    .ui
                                                                    .path
                                                                    .route_query_only_single((
                                                                        SETTINGS_UI_PAGE_QUERY
                                                                            .to_string(),
                                                                        s.to_string(),
                                                                    ));
                                                            }
                                                        };
                                                        add_btn("General");
                                                        add_btn("Language");
                                                        add_btn("Player");
                                                        add_btn("Tee");
                                                        add_btn("Controls");
                                                        add_btn("Graphics");
                                                        add_btn("Sound");
                                                        add_btn("Assets");
                                                    },
                                                );
                                            });
                                            strip.empty();
                                        });
                                });
                            });
                    }
                });
            });
    }
}
