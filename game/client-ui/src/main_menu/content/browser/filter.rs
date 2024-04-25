use egui::{Align, Layout};
use egui_extras::{Size, StripBuilder};

use ui_base::{
    components::menu_top_button::text_icon,
    types::{UIPipe, UIState},
};

use crate::main_menu::user_data::UserData;

/// button & popover
pub fn render(ui: &mut egui::Ui, pipe: &mut UIPipe<UserData>, ui_state: &mut UIState) {
    let search_width = if ui.available_width() < 350.0 {
        150.0
    } else {
        250.0
    };
    let extra_space = 0.0;
    StripBuilder::new(ui)
        .size(Size::exact(extra_space))
        .size(Size::exact(30.0))
        .size(Size::remainder().at_least(search_width))
        .size(Size::exact(30.0))
        .size(Size::exact(extra_space))
        .horizontal(|mut strip| {
            strip.empty();
            strip.cell(|ui| {
                ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                    let text = text_icon(ui, "\u{f0c9}");
                    ui.button(text);
                });
            });
            strip.cell(|ui| {
                StripBuilder::new(ui)
                    .size(Size::remainder())
                    .size(Size::exact(search_width))
                    .size(Size::remainder())
                    .horizontal(|mut strip| {
                        strip.empty();
                        strip.cell(|ui| {
                            ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                                super::search::render(ui, pipe, ui_state);
                            });
                        });
                        strip.empty();
                    });
            });
            strip.cell(|ui| {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let text = text_icon(ui, "\u{f0b0}");
                    ui.button(text);
                });
            });
            strip.empty();
        });
}
