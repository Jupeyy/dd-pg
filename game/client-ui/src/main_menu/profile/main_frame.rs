use egui::{epaint::RectShape, Color32, Frame, Shape};
use egui_extras::{Size, StripBuilder};

use ui_base::{
    types::{UiRenderPipe, UiState},
    utils::add_horizontal_margins,
};

use crate::main_menu::{constants::MENU_PROFILE_NAME, user_data::UserData};

use super::constants::PROFILE_PAGE_QUERY;

/// big box, rounded edges
pub fn render(
    ui: &mut egui::Ui,
    pipe: &mut UiRenderPipe<UserData>,
    ui_state: &mut UiState,
    cur_page: &str,
    main_frame_only: bool,
) {
    let profiles = &*pipe.user_data.profiles;
    let tasks = &mut *pipe.user_data.profile_tasks;
    tasks.update();
    let io = &*pipe.user_data.io;
    let path = &mut pipe.user_data.config.engine.ui.path;
    if cur_page == MENU_PROFILE_NAME {
        let cur_sub = path
            .query
            .get(PROFILE_PAGE_QUERY)
            .map(|path| path.as_ref())
            .unwrap_or("")
            .to_string();
        StripBuilder::new(ui)
            .size(Size::remainder())
            .size(Size::exact(350.0))
            .size(Size::remainder())
            .horizontal(|mut strip| {
                strip.empty();
                strip.cell(|ui| {
                    StripBuilder::new(ui)
                        .size(Size::remainder())
                        .size(Size::exact(150.0))
                        .size(Size::remainder())
                        .clip(true)
                        .vertical(|mut strip| {
                            strip.empty();
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
                                                match cur_sub.as_str() {
                                                    "login-email" => {
                                                        super::login_email::render(
                                                            ui, profiles, tasks, io, path,
                                                        );
                                                    }
                                                    "register-email" => {
                                                        super::register_email::render(
                                                            ui, profiles, tasks, io, path,
                                                        );
                                                    }
                                                    _ => {
                                                        super::overview::render(
                                                            ui, profiles, &tasks, path,
                                                        );
                                                    }
                                                }
                                            });
                                        });
                                }
                            });
                            strip.empty();
                        });
                });
                strip.empty();
            });
    }
}
