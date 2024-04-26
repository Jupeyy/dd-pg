use egui::{vec2, Layout};

use crate::style::default_style;

use super::menu_top_button::text_icon;

pub fn clearable_edit_field(ui: &mut egui::Ui, text: &mut String) -> egui::InnerResponse<()> {
    ui.with_layout(Layout::left_to_right(egui::Align::Center), |ui| {
        let mut style = default_style();
        style.spacing.item_spacing = vec2(0.0, 0.0);
        ui.set_style(style);
        let address = text;
        ui.text_edit_singleline(address);
        let text = text_icon(ui, "\u{f00d}");
        if ui.button(text).clicked() {
            address.clear();
        }
    })
}
