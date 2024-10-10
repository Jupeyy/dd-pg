use egui::{epaint::RectShape, Color32, Layout, RichText};

use ui_base::utils::add_horizontal_margins;

/// can contain various information
/// depends on the modification
/// map name, scorelimit, round
pub fn render(ui: &mut egui::Ui, bottom_label: &str) {
    ui.painter().add(RectShape::filled(
        ui.available_rect_before_wrap(),
        0.0,
        Color32::from_rgba_unmultiplied(70, 70, 70, 255),
    ));
    const FONT_SIZE: f32 = 8.0;
    add_horizontal_margins(ui, |ui| {
        ui.with_layout(Layout::left_to_right(egui::Align::Center), |ui| {
            ui.label(RichText::new(bottom_label).size(FONT_SIZE));
        });
    });
}
