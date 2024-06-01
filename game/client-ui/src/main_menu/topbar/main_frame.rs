use egui::{epaint::RectShape, Color32, Frame, Layout, Shape};

use ui_base::{
    components::menu_top_button::{menu_top_button, menu_top_button_icon, MenuTopButtonProps},
    style::topbar_buttons,
    types::{UiRenderPipe, UiState},
    utils::add_horizontal_margins,
};

use crate::{
    events::UiEvent,
    main_menu::{
        constants::{MENU_PROFILE_NAME, MENU_QUIT_NAME, MENU_SETTINGS_NAME, MENU_UI_PAGE_QUERY},
        user_data::UserData,
    },
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
                            .config
                            .engine
                            .ui
                            .path
                            .query
                            .get(MENU_UI_PAGE_QUERY)
                            .map(|s| {
                                if s.is_empty() {
                                    "".to_string()
                                } else {
                                    s.clone()
                                }
                            });
                        if !pipe.user_data.render_options.hide_buttons_icons {
                            if menu_top_button_icon(
                                ui,
                                MenuTopButtonProps::new(MENU_PROFILE_NAME, &current_active),
                            )
                            .clicked()
                            {
                                pipe.user_data
                                    .config
                                    .engine
                                    .ui
                                    .path
                                    .route_query_only_single((
                                        MENU_UI_PAGE_QUERY.to_string(),
                                        MENU_PROFILE_NAME.to_string(),
                                    ));
                            }
                        }

                        if menu_top_button(
                            ui,
                            MenuTopButtonProps::new(
                                "Internet",
                                &(current_active.clone().or(Some("Internet".to_string()))),
                            ),
                        )
                        .clicked()
                        {
                            pipe.user_data
                                .config
                                .engine
                                .ui
                                .path
                                .route_query_only_single((
                                    MENU_UI_PAGE_QUERY.to_string(),
                                    "Internet".to_string(),
                                ));
                        }
                        if menu_top_button(ui, MenuTopButtonProps::new("LAN", &current_active))
                            .clicked()
                        {
                            pipe.user_data
                                .config
                                .engine
                                .ui
                                .path
                                .route_query_only_single((
                                    MENU_UI_PAGE_QUERY.to_string(),
                                    "LAN".to_string(),
                                ));
                        }
                        if menu_top_button(
                            ui,
                            MenuTopButtonProps::new("Favorites", &current_active),
                        )
                        .clicked()
                        {
                            pipe.user_data
                                .config
                                .engine
                                .ui
                                .path
                                .route_query_only_single((
                                    MENU_UI_PAGE_QUERY.to_string(),
                                    "Favorites".to_string(),
                                ));
                        }
                        if menu_top_button(ui, MenuTopButtonProps::new("DDNet", &current_active))
                            .clicked()
                        {
                            pipe.user_data
                                .config
                                .engine
                                .ui
                                .path
                                .route_query_only_single((
                                    MENU_UI_PAGE_QUERY.to_string(),
                                    "DDNet".to_string(),
                                ));
                        }
                        if menu_top_button(
                            ui,
                            MenuTopButtonProps::new("Community", &current_active),
                        )
                        .clicked()
                        {
                            pipe.user_data
                                .config
                                .engine
                                .ui
                                .path
                                .route_query_only_single((
                                    MENU_UI_PAGE_QUERY.to_string(),
                                    "Community".to_string(),
                                ));
                        }
                        if !pipe.user_data.render_options.hide_buttons_icons {
                            ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                                if menu_top_button_icon(
                                    ui,
                                    MenuTopButtonProps::new(MENU_QUIT_NAME, &current_active),
                                )
                                .clicked()
                                {
                                    pipe.user_data.events.push(UiEvent::Quit);
                                }
                                if menu_top_button_icon(
                                    ui,
                                    MenuTopButtonProps::new(MENU_SETTINGS_NAME, &current_active),
                                )
                                .clicked()
                                {
                                    pipe.user_data
                                        .config
                                        .engine
                                        .ui
                                        .path
                                        .route_query_only_single((
                                            MENU_UI_PAGE_QUERY.to_string(),
                                            MENU_SETTINGS_NAME.to_string(),
                                        ));
                                }
                                if menu_top_button_icon(
                                    ui,
                                    MenuTopButtonProps::new("\u{e131}", &current_active),
                                )
                                .clicked()
                                {
                                    pipe.user_data
                                        .config
                                        .engine
                                        .ui
                                        .path
                                        .route_query_only_single((
                                            "demo".to_string(),
                                            '\u{e131}'.to_string(),
                                        ));

                                    pipe.user_data.events.push(UiEvent::StartDemo {
                                        name: "demo.twdemo".to_string(),
                                    })
                                }
                                if menu_top_button_icon(
                                    ui,
                                    MenuTopButtonProps::new("\u{f279}", &current_active),
                                )
                                .clicked()
                                {
                                    pipe.user_data.events.push(UiEvent::StartEditor)
                                }
                            });
                        }
                    });
                });
            });
    }
}
