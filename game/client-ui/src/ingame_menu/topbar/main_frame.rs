use egui::{epaint::RectShape, Color32, Frame, Layout, Shape};

use ui_base::{
    components::menu_top_button::{menu_top_button, menu_top_button_icon, MenuTopButtonProps},
    style::topbar_buttons,
    types::{UiRenderPipe, UiState},
    utils::add_horizontal_margins,
};

use crate::{
    events::UiEvent,
    ingame_menu::{constants::INGAME_MENU_UI_PAGE_QUERY, user_data::UserData},
    main_menu::constants::{MENU_QUIT_NAME, MENU_SETTINGS_NAME, MENU_UI_PAGE_QUERY},
};

/// main frame. full width
pub fn render(
    ui: &mut egui::Ui,
    pipe: &mut UiRenderPipe<UserData>,
    _ui_state: &mut UiState,
    main_frame_only: bool,
) {
    if main_frame_only {
        ui.painter().add(Shape::Rect(RectShape::filled(
            ui.available_rect_before_wrap(),
            0.0,
            Color32::from_rgba_unmultiplied(0, 0, 0, 255),
        )));
    } else {
        Frame::default()
            .fill(Color32::from_rgba_unmultiplied(0, 0, 0, 100))
            .show(ui, |ui| {
                add_horizontal_margins(ui, |ui| {
                    ui.set_style(topbar_buttons());
                    ui.horizontal(|ui| {
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
                            });
                        if menu_top_button(ui, MenuTopButtonProps::new("Game", &current_active))
                            .clicked()
                        {
                            pipe.user_data
                                .browser_menu
                                .config
                                .engine
                                .ui
                                .path
                                .route_query_only_single((
                                    INGAME_MENU_UI_PAGE_QUERY.to_string(),
                                    "Game".to_string(),
                                ));
                        }
                        if menu_top_button(ui, MenuTopButtonProps::new("Players", &current_active))
                            .clicked()
                        {
                            pipe.user_data
                                .browser_menu
                                .config
                                .engine
                                .ui
                                .path
                                .route_query_only_single((
                                    INGAME_MENU_UI_PAGE_QUERY.to_string(),
                                    "Players".to_string(),
                                ));
                        }
                        if menu_top_button(
                            ui,
                            MenuTopButtonProps::new("Server info", &current_active),
                        )
                        .clicked()
                        {
                            pipe.user_data
                                .browser_menu
                                .config
                                .engine
                                .ui
                                .path
                                .route_query_only_single((
                                    INGAME_MENU_UI_PAGE_QUERY.to_string(),
                                    "Server info".to_string(),
                                ));
                        }
                        if menu_top_button(ui, MenuTopButtonProps::new("Browser", &current_active))
                            .clicked()
                        {
                            pipe.user_data
                                .browser_menu
                                .config
                                .engine
                                .ui
                                .path
                                .route_query_only_single((
                                    INGAME_MENU_UI_PAGE_QUERY.to_string(),
                                    "Browser".to_string(),
                                ));
                        }
                        if menu_top_button(ui, MenuTopButtonProps::new("Ghost", &current_active))
                            .clicked()
                        {
                            pipe.user_data
                                .browser_menu
                                .config
                                .engine
                                .ui
                                .path
                                .route_query_only_single((
                                    INGAME_MENU_UI_PAGE_QUERY.to_string(),
                                    "Ghost".to_string(),
                                ));
                        }
                        if menu_top_button(
                            ui,
                            MenuTopButtonProps::new("Call vote", &current_active),
                        )
                        .clicked()
                        {
                            pipe.user_data
                                .browser_menu
                                .config
                                .engine
                                .ui
                                .path
                                .route_query_only_single((
                                    INGAME_MENU_UI_PAGE_QUERY.to_string(),
                                    "Call vote".to_string(),
                                ));
                        }
                        ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                            if menu_top_button_icon(
                                ui,
                                MenuTopButtonProps::new(MENU_QUIT_NAME, &current_active),
                            )
                            .clicked()
                            {
                                pipe.user_data.browser_menu.events.push(UiEvent::Quit);
                            }
                            if menu_top_button_icon(
                                ui,
                                MenuTopButtonProps::new(MENU_SETTINGS_NAME, &current_active),
                            )
                            .clicked()
                            {
                                pipe.user_data
                                    .browser_menu
                                    .config
                                    .engine
                                    .ui
                                    .path
                                    .route_query_only_single((
                                        INGAME_MENU_UI_PAGE_QUERY.to_string(),
                                        MENU_SETTINGS_NAME.to_string(),
                                    ));
                                pipe.user_data
                                    .browser_menu
                                    .config
                                    .engine
                                    .ui
                                    .path
                                    .route_query_only_single((
                                        MENU_UI_PAGE_QUERY.to_string(),
                                        MENU_SETTINGS_NAME.to_string(),
                                    ));
                            }
                        });
                    });
                });
            });
    }
}
