use egui::{epaint::RectShape, Color32, Frame, Shape};

use ui_base::{
    components::menu_top_button::{menu_top_button, MenuTopButtonProps},
    style::topbar_buttons,
    types::UiRenderPipe,
    utils::add_horizontal_margins,
};

use crate::{
    ingame_menu::{constants::INGAME_MENU_UI_PAGE_QUERY, user_data::UserData},
    main_menu::{constants::MENU_UI_PAGE_QUERY, topbar::main_frame::render_right_buttons},
};

/// main frame. full width
pub fn render(ui: &mut egui::Ui, pipe: &mut UiRenderPipe<UserData>, main_frame_only: bool) {
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
                        if menu_top_button(
                            ui,
                            |_, _| None,
                            MenuTopButtonProps::new("Game", &current_active),
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
                                    "Game".to_string(),
                                ));
                        }
                        if menu_top_button(
                            ui,
                            |_, _| None,
                            MenuTopButtonProps::new("Players", &current_active),
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
                                    "Players".to_string(),
                                ));
                        }

                        if pipe
                            .user_data
                            .game_server_info
                            .server_options()
                            .use_account_name
                            && menu_top_button(
                                ui,
                                |_, _| None,
                                MenuTopButtonProps::new("Account", &current_active),
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
                                    "Account".to_string(),
                                ));
                        }
                        if menu_top_button(
                            ui,
                            |_, _| None,
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
                        if menu_top_button(
                            ui,
                            |_, _| None,
                            MenuTopButtonProps::new("Browser", &current_active),
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
                                    "Browser".to_string(),
                                ));
                        }
                        if menu_top_button(
                            ui,
                            |_, _| None,
                            MenuTopButtonProps::new("Ghost", &current_active),
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
                                    "Ghost".to_string(),
                                ));
                        }
                        if menu_top_button(
                            ui,
                            |_, _| None,
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
                        render_right_buttons(
                            ui,
                            pipe.user_data.browser_menu.events,
                            pipe.user_data.browser_menu.config,
                            pipe.user_data.browser_menu.main_menu,
                            &current_active,
                            &[INGAME_MENU_UI_PAGE_QUERY, MENU_UI_PAGE_QUERY],
                        );
                    });
                });
            });
    }
}
