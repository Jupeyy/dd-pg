use egui::{epaint::RectShape, Shape};
use egui_extras::{Column, Size, StripBuilder, TableBuilder};
use graphics::graphics::GraphicsBase;
use graphics_backend_traits::traits::GraphicsBackendInterface;
use ui_base::types::{UIPipe, UIState};

use crate::main_menu::user_data::UserData;

/// table header + server list
pub fn render<B: GraphicsBackendInterface>(
    ui: &mut egui::Ui,
    pipe: &mut UIPipe<UserData>,
    ui_state: &mut UIState,
    graphics: &mut GraphicsBase<B>,
) {
    StripBuilder::new(ui)
        .size(Size::exact(0.0))
        .size(Size::remainder())
        .size(Size::exact(0.0))
        .clip(true)
        .horizontal(|mut strip| {
            strip.empty();
            strip.cell(|ui| {
                ui.painter().add(Shape::Rect(RectShape::filled(
                    ui.available_rect_before_wrap(),
                    0.0,
                    ui.style().visuals.window_fill,
                )));
                egui::ScrollArea::horizontal().show(ui, |ui| {
                    let height = ui.available_height();
                    let width = ui.available_width();
                    ui.push_id(
                        "server-browser-server-list".to_string() + &width.to_string(),
                        |ui| {
                            let table = TableBuilder::new(ui)
                                .min_scrolled_height(50.0)
                                .max_scroll_height(height)
                                .column(Column::exact(30.0).clip(true).resizable(false))
                                .column(Column::remainder().clip(true))
                                .column(Column::remainder().clip(true))
                                .column(Column::remainder().clip(true))
                                .column(Column::exact(60.0).clip(true))
                                .column(Column::exact(30.0).clip(true))
                                .resizable(true)
                                .striped(true)
                                .cell_layout(egui::Layout::left_to_right(egui::Align::Center));
                            table
                                .header(20.0, |mut header| {
                                    super::header::render(&mut header, pipe, ui_state, graphics);
                                })
                                .body(|body| {
                                    super::server_list::frame::render(
                                        body, pipe, ui_state, graphics,
                                    );
                                });
                        },
                    );
                });
            });
            strip.empty();
        });
}
