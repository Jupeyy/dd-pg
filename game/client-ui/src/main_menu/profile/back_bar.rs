use config::config::ConfigPath;
use egui_extras::{Size, StripBuilder};
use ui_base::utils::icon_font_text;

use super::constants::PROFILE_PAGE_QUERY;

pub fn back_bar(ui: &mut egui::Ui, title: &str, path: &mut ConfigPath) {
    ui.horizontal(|ui| {
        StripBuilder::new(ui)
            .size(Size::exact(20.0))
            .size(Size::remainder())
            .size(Size::exact(20.0))
            .horizontal(|mut strip| {
                strip.cell(|ui| {
                    if ui.button(icon_font_text(ui, "\u{f060}")).clicked() {
                        path.query
                            .insert(PROFILE_PAGE_QUERY.to_string(), "overview".into());
                    }
                });
                strip.cell(|ui| {
                    ui.vertical_centered(|ui: &mut egui::Ui| {
                        ui.label(title);
                    });
                });
                strip.empty();
            });
    });
}
