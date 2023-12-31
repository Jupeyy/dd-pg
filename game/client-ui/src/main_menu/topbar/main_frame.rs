use egui::{epaint::RectShape, Color32, Layout, Shape};
use graphics::graphics::Graphics;
use ui_base::{
    components::menu_top_button::{menu_top_button, menu_top_button_icon, MenuTopButtonProps},
    style::topbar_buttons,
    types::{UIPipe, UIState},
};

use crate::main_menu::user_data::UserData;

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
            let current_active = pipe.config.ui.path.query.get("main").map(|s| {
                if s.is_empty() {
                    "".to_string()
                } else {
                    s.clone()
                }
            });
            if menu_top_button(ui, MenuTopButtonProps::new("Internet", &current_active)).clicked() {
                pipe.config
                    .ui
                    .path
                    .route_query_only_single(("main".to_string(), "Internet".to_string()));
            }
            if menu_top_button(ui, MenuTopButtonProps::new("LAN", &current_active)).clicked() {
                pipe.config
                    .ui
                    .path
                    .route_query_only_single(("main".to_string(), "LAN".to_string()));
            }
            if menu_top_button(ui, MenuTopButtonProps::new("Favorites", &current_active)).clicked()
            {
                pipe.config
                    .ui
                    .path
                    .route_query_only_single(("main".to_string(), "Favorites".to_string()));
            }
            if menu_top_button(ui, MenuTopButtonProps::new("DDNet", &current_active)).clicked() {
                pipe.config
                    .ui
                    .path
                    .route_query_only_single(("main".to_string(), "DDNet".to_string()));
            }
            if menu_top_button(ui, MenuTopButtonProps::new("Community", &current_active)).clicked()
            {
                pipe.config
                    .ui
                    .path
                    .route_query_only_single(("main".to_string(), "Community".to_string()));
            }
            if !pipe.user_data.render_options.hide_buttons_right {
                ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                    if menu_top_button_icon(
                        ui,
                        MenuTopButtonProps::new("\u{f011}", &current_active),
                    )
                    .clicked()
                    {
                        // TODO: quit
                    }
                    if menu_top_button_icon(
                        ui,
                        MenuTopButtonProps::new("\u{f013}", &current_active),
                    )
                    .clicked()
                    {
                        pipe.config
                            .ui
                            .path
                            .route_query_only_single(("main".to_string(), '\u{f013}'.to_string()));
                    }
                });
            }
        });
    }
}
