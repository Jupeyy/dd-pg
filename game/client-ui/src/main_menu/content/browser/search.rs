use egui_extras::{Size, StripBuilder};

use ui_base::{
    components::clearable_edit_field::clearable_edit_field, types::UiRenderPipe,
    utils::icon_font_text_for_btn,
};

use crate::main_menu::user_data::UserData;

fn exclude_menu(ui: &mut egui::Ui, pipe: &mut UiRenderPipe<UserData>) {
    ui.label("Exclude words\n(seperated by \";\")");
    ui.text_edit_singleline(pipe.user_data.config.storage_entry("filter.exclude"));
}

/// search field
pub fn render(ui: &mut egui::Ui, pipe: &mut UiRenderPipe<UserData>) {
    ui.style_mut().spacing.item_spacing.x = 2.0;
    StripBuilder::new(ui)
        .size(Size::exact(20.0))
        .size(Size::remainder())
        .size(Size::exact(20.0))
        .horizontal(|mut strip| {
            strip.cell(|ui| {
                // search icon
                ui.label(icon_font_text_for_btn(ui, "\u{f002}"));
            });
            strip.cell(|ui| {
                clearable_edit_field(
                    ui,
                    pipe.user_data.config.storage_entry("filter.search"),
                    None,
                    None,
                );
            });
            strip.cell(|ui| {
                // exclude
                ui.menu_button(icon_font_text_for_btn(ui, "\u{f05e}"), |ui| {
                    exclude_menu(ui, pipe)
                });
            });
        });
}
