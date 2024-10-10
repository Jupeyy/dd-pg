use egui_extras::{Size, StripBuilder};

use ui_base::types::{UiRenderPipe, UiState};

use super::{constants::MENU_UI_PAGE_QUERY, user_data::UserData};

/// big square, rounded edges
pub fn render(
    ui: &mut egui::Ui,
    pipe: &mut UiRenderPipe<UserData>,
    ui_state: &mut UiState,
    main_frame_only: bool,
) {
    StripBuilder::new(ui)
        .size(Size::exact(20.0))
        .size(Size::exact(10.0))
        .size(Size::remainder())
        .size(Size::exact(10.0))
        .vertical(|mut strip| {
            strip.cell(|ui| {
                super::topbar::main_frame::render(ui, pipe, ui_state, main_frame_only);
            });
            strip.empty();
            strip.strip(|builder| {
                builder
                    .size(Size::exact(10.0))
                    .size(Size::remainder())
                    .size(Size::exact(10.0))
                    .horizontal(|mut strip| {
                        strip.empty();
                        strip.cell(|ui| {
                            let cur_page = pipe
                                .user_data
                                .config
                                .engine
                                .ui
                                .path
                                .query
                                .get(MENU_UI_PAGE_QUERY)
                                .map(|path| path.as_ref())
                                .unwrap_or("")
                                .to_string();
                            super::content::main_frame::render(
                                ui,
                                pipe,
                                ui_state,
                                &cur_page,
                                main_frame_only,
                            );
                            super::settings::main_frame::render(
                                ui,
                                pipe,
                                ui_state,
                                &cur_page,
                                main_frame_only,
                            );
                            super::demo::main_frame::render(ui, pipe, &cur_page, main_frame_only);
                            super::profile::main_frame::render(
                                ui,
                                pipe,
                                &cur_page,
                                main_frame_only,
                            );
                        });
                        strip.empty();
                    });
            });
            strip.empty();
        });
}
