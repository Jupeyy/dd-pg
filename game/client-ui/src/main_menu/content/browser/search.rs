use egui::Layout;
use egui_extras::{Size, StripBuilder};
use graphics::graphics::Graphics;
use ui_base::{
    components::{clearable_edit_field::clearable_edit_field, menu_top_button::text_icon},
    types::{UIPipe, UIState},
};

use crate::main_menu::user_data::UserData;

fn exclude_menu(ui: &mut egui::Ui, pipe: &mut UIPipe<UserData>) {
    ui.label("Exclude words\n(seperated by \";\")");
    ui.text_edit_singleline(&mut pipe.user_data.browser_data.filter.exclude);
}

/// search field
pub fn render(
    ui: &mut egui::Ui,
    pipe: &mut UIPipe<UserData>,
    _ui_state: &mut UIState,
    _graphics: &mut Graphics,
) {
    StripBuilder::new(ui)
        .size(Size::exact(20.0))
        .size(Size::remainder())
        .size(Size::exact(25.0))
        .horizontal(|mut strip| {
            strip.cell(|ui| {
                ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                    let text = text_icon(ui, "\u{f002}");
                    ui.label(text);
                });
            });
            strip.cell(|ui| {
                clearable_edit_field(ui, &mut pipe.user_data.browser_data.filter.search);
            });
            strip.cell(|ui| {
                ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                    let text = text_icon(ui, "\u{f05e}");
                    ui.menu_button(text, |ui| exclude_menu(ui, pipe));
                });
            });
        });
}
