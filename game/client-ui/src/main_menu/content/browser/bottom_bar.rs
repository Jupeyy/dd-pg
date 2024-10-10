use egui::{Align, Layout};
use egui_extras::{Size, StripBuilder};

use ui_base::types::UiRenderPipe;

use crate::main_menu::user_data::UserData;

/// server address, info, connect & refresh button
pub fn render(ui: &mut egui::Ui, pipe: &mut UiRenderPipe<UserData>) {
    let extra_space = 0.0;
    let width = ui.available_width();
    StripBuilder::new(ui)
        .size(Size::exact(extra_space))
        .size(Size::exact(width * 0.6))
        .size(Size::remainder())
        .size(Size::exact(extra_space))
        .clip(true)
        .horizontal(|mut strip| {
            strip.empty();
            strip.cell(|ui| {
                ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                    super::server_address::render(ui, pipe);
                });
            });
            strip.cell(|ui| {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    super::connect_refresh::render(ui, pipe);
                    if ui.available_width() >= 70.0 {
                        super::info::render(ui, pipe);
                    }
                });
            });
            strip.empty();
        });
}
