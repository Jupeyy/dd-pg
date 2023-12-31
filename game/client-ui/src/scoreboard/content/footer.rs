use egui::{epaint::RectShape, Color32, Layout, RichText};
use graphics::graphics::Graphics;
use ui_base::{
    types::{UIPipe, UIState},
    utils::add_horizontal_margins,
};

use crate::scoreboard::user_data::UserData;

/// can contain various information
/// depends on the modification
/// map name, scorelimit, round
pub fn render(
    ui: &mut egui::Ui,
    _pipe: &mut UIPipe<UserData>,
    _ui_state: &mut UIState,
    _graphics: &mut Graphics,
) {
    ui.painter().add(RectShape::filled(
        ui.available_rect_before_wrap(),
        0.0,
        Color32::DARK_GRAY,
    ));
    const FONT_SIZE: f32 = 8.0;
    add_horizontal_margins(ui, |ui| {
        ui.with_layout(Layout::left_to_right(egui::Align::Center), |ui| {
            ui.label(RichText::new("Score limit: 500").size(FONT_SIZE));
        });
    });
}
