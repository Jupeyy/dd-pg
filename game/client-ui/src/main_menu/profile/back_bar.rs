use egui_extras::{Size, StripBuilder};
use ui_base::utils::icon_font_text_for_btn;

use crate::main_menu::user_data::{ProfileState, ProfileTasks};

pub fn back_bar(ui: &mut egui::Ui, title: &str, tasks: &mut ProfileTasks) {
    ui.horizontal(|ui| {
        StripBuilder::new(ui)
            .size(Size::exact(20.0))
            .size(Size::remainder())
            .size(Size::exact(20.0))
            .horizontal(|mut strip| {
                strip.cell(|ui| {
                    if ui.button(icon_font_text_for_btn(ui, "\u{f060}")).clicked() {
                        tasks.state = ProfileState::Overview;
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
