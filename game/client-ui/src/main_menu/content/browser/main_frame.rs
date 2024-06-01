use egui::{epaint::RectShape, Color32, Frame, Shape};
use egui_extras::{Size, StripBuilder};

use ui_base::types::{UiRenderPipe, UiState};

use crate::main_menu::user_data::UserData;

/// big box, rounded edges
pub fn render(
    ui: &mut egui::Ui,
    pipe: &mut UiRenderPipe<UserData>,
    ui_state: &mut UiState,
    cur_page: &str,
    main_frame_only: bool,
) {
    let w = ui.available_width();
    let margin = ui.style().spacing.item_spacing.x;
    let width_details = 200.0;
    let width_browser = w - width_details - margin;
    StripBuilder::new(ui)
        .size(Size::exact(width_browser))
        .size(Size::remainder())
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
                            let filter_height = 30.0;
                            let bottom_bar_height = 30.0;
                            StripBuilder::new(ui)
                                .size(Size::exact(0.0))
                                .size(Size::exact(filter_height))
                                .size(Size::remainder())
                                .size(Size::exact(bottom_bar_height))
                                .size(Size::exact(0.0))
                                .clip(true)
                                .vertical(|mut strip| {
                                    strip.empty();
                                    strip.cell(|ui| {
                                        super::filter::render(ui, pipe, ui_state);
                                    });
                                    strip.cell(|ui| {
                                        super::list::list::render(ui, pipe, ui_state, cur_page);
                                    });
                                    strip.cell(|ui| {
                                        super::bottom_bar::render(ui, pipe, ui_state);
                                    });
                                    strip.empty();
                                });
                        });
                }
            });
            strip.cell(|ui| {
                StripBuilder::new(ui)
                    .size(Size::remainder())
                    .size(Size::exact(0.0))
                    .size(Size::remainder())
                    .vertical(|mut strip| {
                        strip.cell(|ui| {
                            let browser_data = &pipe.user_data.browser_data;
                            if let Some(server) = browser_data
                                .servers
                                .iter()
                                .find(|server| server.address == browser_data.cur_address)
                            {
                                super::info_panel::main_frame::render(
                                    ui,
                                    ui_state,
                                    main_frame_only,
                                    server,
                                );
                            }
                        });
                        strip.empty();
                        strip.cell(|ui| {
                            super::friend_list::main_frame::render(
                                ui,
                                pipe,
                                ui_state,
                                main_frame_only,
                            );
                        });
                    });
            });
        });
}
