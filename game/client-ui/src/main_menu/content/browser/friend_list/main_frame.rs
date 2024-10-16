use egui::{epaint::RectShape, Color32, Frame, Layout, Shape};
use egui_extras::{Size, StripBuilder};

use ui_base::{
    types::{UiRenderPipe, UiState},
    utils::icon_font_plus_text,
};

use crate::main_menu::user_data::UserData;

/// big box, rounded edges
pub fn render(
    ui: &mut egui::Ui,
    pipe: &mut UiRenderPipe<UserData>,
    ui_state: &mut UiState,
    main_frame_only: bool,
) {
    if main_frame_only {
        ui.painter().add(Shape::Rect(RectShape::filled(
            ui.available_rect_before_wrap(),
            5.0,
            Color32::from_rgba_unmultiplied(0, 0, 0, 255),
        )));
    } else {
        Frame::default()
            .fill(Color32::from_rgba_unmultiplied(0, 0, 0, 100))
            .rounding(5.0)
            .show(ui, |ui| {
                let item_spacing = ui.style().spacing.item_spacing.x;
                StripBuilder::new(ui)
                    .size(Size::exact(0.0))
                    .size(Size::remainder())
                    .size(Size::exact(0.0))
                    .horizontal(|mut strip| {
                        strip.empty();
                        strip.cell(|ui| {
                            StripBuilder::new(ui)
                                .size(Size::exact(item_spacing))
                                .size(Size::exact(15.0))
                                .size(Size::exact(item_spacing))
                                .size(Size::remainder())
                                .size(Size::exact(item_spacing))
                                .vertical(|mut strip| {
                                    strip.empty();
                                    strip.cell(|ui| {
                                        ui.with_layout(
                                            Layout::left_to_right(egui::Align::Center)
                                                .with_main_align(egui::Align::Center)
                                                .with_main_justify(true),
                                            |ui| {
                                                ui.label(icon_font_plus_text(
                                                    ui,
                                                    "\u{e533}",
                                                    "Friends & Favorites",
                                                ));
                                            },
                                        );
                                    });
                                    strip.empty();
                                    strip.cell(|ui| {
                                        super::table::render(ui, pipe, ui_state);
                                    });
                                    strip.empty();
                                });
                        });
                        strip.empty();
                    });
            });
    }
}
