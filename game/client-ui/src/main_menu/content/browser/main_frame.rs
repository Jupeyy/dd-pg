use egui::{epaint::RectShape, Shape};
use egui_extras::{Size, StripBuilder};
use graphics::graphics::GraphicsBase;
use graphics_backend_traits::traits::GraphicsBackendInterface;
use ui_base::types::{UIPipe, UIState};

use crate::main_menu::user_data::UserData;

/// big box, rounded edges
pub fn render<B: GraphicsBackendInterface>(
    ui: &mut egui::Ui,
    pipe: &mut UIPipe<UserData>,
    ui_state: &mut UIState,
    graphics: &mut GraphicsBase<B>,
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
                        ui.style().visuals.window_fill,
                    )));
                } else {
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
                                super::filter::render(ui, pipe, ui_state, graphics);
                            });
                            strip.cell(|ui| {
                                super::list::list::render(ui, pipe, ui_state, graphics);
                            });
                            strip.cell(|ui| {
                                super::bottom_bar::render(ui, pipe, ui_state, graphics);
                            });
                            strip.empty();
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
                            super::info_panel::main_frame::render(
                                ui,
                                pipe,
                                ui_state,
                                graphics,
                                main_frame_only,
                            );
                        });
                        strip.empty();
                        strip.cell(|ui| {
                            super::friend_list::main_frame::render(
                                ui,
                                pipe,
                                ui_state,
                                graphics,
                                main_frame_only,
                            );
                        });
                    });
            });
        });
}
