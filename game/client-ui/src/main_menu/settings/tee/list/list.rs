use std::collections::BTreeMap;

use client_containers::container::ContainerItemIndexType;
use egui::{epaint::RectShape, Color32, Layout, ScrollArea, Shape};
use ui_base::{
    types::{UiRenderPipe, UiState},
    utils::add_horizontal_margins,
};

use crate::main_menu::user_data::UserData;

pub fn render(
    ui: &mut egui::Ui,
    pipe: &mut UiRenderPipe<UserData>,
    ui_state: &mut UiState,
    entries: BTreeMap<String, ContainerItemIndexType>,
    player_index: usize,
) {
    ui.painter().add(Shape::Rect(RectShape::filled(
        ui.available_rect_before_wrap(),
        0.0,
        Color32::from_rgba_unmultiplied(0, 0, 0, 100),
    )));
    ui.style_mut().spacing.scroll.floating = false;
    ScrollArea::vertical().show(ui, |ui| {
        add_horizontal_margins(ui, |ui| {
            ui.with_layout(
                Layout::left_to_right(egui::Align::Min)
                    .with_main_wrap(true)
                    .with_main_align(egui::Align::Min),
                |ui| {
                    for (skin, ty) in entries {
                        super::entry::render(ui, &skin, ty, player_index, pipe, ui_state);
                    }
                },
            );
        });
    });
}
