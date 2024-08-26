use egui::{epaint::RectShape, Color32, Sense, Shape};
use egui_extras::{Column, Size, StripBuilder, TableBuilder};

use ui_base::types::{UiRenderPipe, UiState};

use crate::main_menu::user_data::UserData;

/// table header + server list
pub fn render(
    ui: &mut egui::Ui,
    pipe: &mut UiRenderPipe<UserData>,
    ui_state: &mut UiState,
    cur_page: &str,
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
                    Color32::from_rgba_unmultiplied(0, 0, 0, 100),
                )));
                egui::ScrollArea::horizontal().show(ui, |ui| {
                    let height = ui.available_height();
                    let width = ui.available_width();
                    ui.push_id(format!("server-browser-server-list{width}"), |ui| {
                        ui.style_mut().spacing.scroll.floating = false;
                        let table = TableBuilder::new(ui)
                            .min_scrolled_height(50.0)
                            .max_scroll_height(height)
                            .column(Column::exact(30.0).clip(true).resizable(false))
                            .column(Column::remainder().clip(true))
                            .column(Column::remainder().clip(true))
                            .column(Column::remainder().clip(true))
                            .column(Column::exact(60.0).clip(true))
                            .column(Column::exact(30.0).clip(true))
                            .resizable(false)
                            .striped(true)
                            .sense(Sense::click())
                            .cell_layout(egui::Layout::left_to_right(egui::Align::Center));
                        table
                            .header(20.0, |mut header| {
                                super::header::render(&mut header, pipe, ui_state);
                            })
                            .body(|body| {
                                super::server_list::frame::render(body, pipe, ui_state, cur_page);
                            });
                    });
                });
            });
            strip.empty();
        });
}
