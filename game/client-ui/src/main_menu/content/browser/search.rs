use egui_extras::{Size, StripBuilder};

use ui_base::{
    components::{clearable_edit_field::clearable_edit_field, menu_top_button::text_icon},
    types::UiRenderPipe,
};

use crate::main_menu::user_data::UserData;

fn exclude_menu(ui: &mut egui::Ui, pipe: &mut UiRenderPipe<UserData>) {
    ui.label("Exclude words\n(seperated by \";\")");
    ui.text_edit_singleline(&mut pipe.user_data.browser_data.filter.exclude);
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
                let text = text_icon(ui, "\u{f002}");
                ui.label(text);
            });
            strip.cell(|ui| {
                clearable_edit_field(ui, &mut pipe.user_data.browser_data.filter.search, None);
            });
            strip.cell(|ui| {
                let text = text_icon(ui, "\u{f05e}");
                ui.menu_button(text, |ui| exclude_menu(ui, pipe));
            });
        });
}
