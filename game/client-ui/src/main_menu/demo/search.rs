use egui::{Align, Layout};
use egui_extras::{Size, StripBuilder};

use ui_base::{components::clearable_edit_field::clearable_edit_field, types::UiRenderPipe};

use crate::main_menu::user_data::UserData;

/// search field
pub fn render(ui: &mut egui::Ui, pipe: &mut UiRenderPipe<UserData>) {
    ui.style_mut().spacing.item_spacing.x = 2.0;
    StripBuilder::new(ui)
        .size(Size::remainder())
        .size(Size::exact(250.0))
        .size(Size::remainder())
        .horizontal(|mut strip| {
            strip.empty();
            strip.cell(|ui| {
                ui.style_mut().wrap_mode = None;
                ui.with_layout(
                    Layout::left_to_right(Align::Center)
                        .with_main_align(Align::Center)
                        .with_main_justify(true),
                    |ui| {
                        StripBuilder::new(ui)
                            .size(Size::exact(20.0))
                            .size(Size::remainder())
                            .horizontal(|mut strip| {
                                strip.cell(|ui| {
                                    ui.style_mut().wrap_mode = None;
                                    // search icon
                                    ui.label("\u{1f50d}");
                                });
                                strip.cell(|ui| {
                                    ui.style_mut().wrap_mode = None;
                                    clearable_edit_field(
                                        ui,
                                        pipe.user_data.config.storage_entry("demo.search"),
                                        None,
                                        None,
                                    );
                                });
                            });
                    },
                );
            });
            strip.empty();
        });
}
