use egui::{epaint::RectShape, Shape};
use egui_extras::{Column, TableBuilder};
use graphics::graphics::Graphics;
use ui_base::types::{UIPipe, UIState};

use crate::main_menu::user_data::UserData;

/// table header + server list
pub fn render(
    ui: &mut egui::Ui,
    pipe: &mut UIPipe<UserData>,
    ui_state: &mut UIState,
    graphics: &mut Graphics,
) {
    ui.push_id("player-list", |ui| {
        egui::ScrollArea::horizontal().show(ui, |ui| {
            ui.painter().add(Shape::Rect(RectShape::filled(
                ui.available_rect_before_wrap(),
                0.0,
                ui.style().visuals.window_fill,
            )));
            let height = ui.available_height();
            let table = TableBuilder::new(ui)
                .min_scrolled_height(10.0)
                .max_scroll_height(height)
                .column(Column::remainder().clip(true))
                .column(Column::remainder().clip(true))
                .column(Column::remainder().clip(true))
                .resizable(false)
                .striped(true)
                .cell_layout(egui::Layout::left_to_right(egui::Align::Center));
            table.body(|body| {
                super::list::frame::render(body, pipe, ui_state, graphics);
            });
        });
    });
}
