use egui::{epaint::RectShape, Color32, Layout, RichText};

use ui_base::{
    types::{UiRenderPipe, UiState},
    utils::add_horizontal_margins,
};

use crate::scoreboard::user_data::UserData;

/// can contain various information
/// depends on the modification
/// map name, scorelimit, round
pub fn render(ui: &mut egui::Ui, _pipe: &mut UiRenderPipe<UserData>, _ui_state: &mut UiState) {
    ui.painter().add(RectShape::filled(
        ui.available_rect_before_wrap(),
        0.0,
        Color32::from_rgba_unmultiplied(70, 70, 70, 255),
    ));
    const FONT_SIZE: f32 = 8.0;
    add_horizontal_margins(ui, |ui| {
        ui.with_layout(Layout::left_to_right(egui::Align::Center), |ui| {
            ui.label(RichText::new("Score limit: 500").size(FONT_SIZE));
        });
    });
}
