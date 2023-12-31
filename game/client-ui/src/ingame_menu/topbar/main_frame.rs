use egui::{epaint::RectShape, Color32, Layout, Shape};
use graphics::graphics::Graphics;
use ui_base::{
    components::menu_top_button::{menu_top_button, menu_top_button_icon, MenuTopButtonProps},
    style::topbar_buttons,
    types::{UIPipe, UIState},
};

use crate::ingame_menu::user_data::UserData;

/// main frame. full width
pub fn render(
    ui: &mut egui::Ui,
    pipe: &mut UIPipe<UserData>,
    _ui_state: &mut UIState,
    _graphics: &mut Graphics,
    main_frame_only: bool,
) {
    if main_frame_only {
        ui.painter().add(Shape::Rect(RectShape::filled(
            ui.available_rect_before_wrap(),
            0.0,
            Color32::from_rgba_unmultiplied(0, 0, 0, 255),
        )));
    } else {
        ui.set_style(topbar_buttons());
        ui.horizontal(|ui| {
            let current_active = pipe.config.ui.path.query.get("game").map(|s| {
                if s.is_empty() {
                    "".to_string()
                } else {
                    s.clone()
                }
            });
            if menu_top_button(ui, MenuTopButtonProps::new("Game", &current_active)).clicked() {
                pipe.config
                    .ui
                    .path
                    .route_query_only_single(("game".to_string(), "Game".to_string()));
            }
            if menu_top_button(ui, MenuTopButtonProps::new("Players", &current_active)).clicked() {
                pipe.config
                    .ui
                    .path
                    .route_query_only_single(("game".to_string(), "Players".to_string()));
            }
            if menu_top_button(ui, MenuTopButtonProps::new("Server info", &current_active))
                .clicked()
            {
                pipe.config
                    .ui
                    .path
                    .route_query_only_single(("game".to_string(), "Server info".to_string()));
            }
            if menu_top_button(ui, MenuTopButtonProps::new("Browser", &current_active)).clicked() {
                pipe.config
                    .ui
                    .path
                    .route_query_only_single(("game".to_string(), "Browser".to_string()));
            }
            if menu_top_button(ui, MenuTopButtonProps::new("Ghost", &current_active)).clicked() {
                pipe.config
                    .ui
                    .path
                    .route_query_only_single(("game".to_string(), "Ghost".to_string()));
            }
            if menu_top_button(ui, MenuTopButtonProps::new("Call vote", &current_active)).clicked()
            {
                pipe.config
                    .ui
                    .path
                    .route_query_only_single(("game".to_string(), "Call vote".to_string()));
            }
            ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                if menu_top_button_icon(ui, MenuTopButtonProps::new("\u{f011}", &current_active))
                    .clicked()
                {
                    // TODO: quit
                }
                if menu_top_button_icon(ui, MenuTopButtonProps::new("\u{f013}", &current_active))
                    .clicked()
                {
                    pipe.config
                        .ui
                        .path
                        .route_query_only_single(("main".to_string(), '\u{f013}'.to_string()));
                }
            });
        });
    }
}
