use egui::{epaint::RectShape, FontId, Layout, RichText, Shape, TextFormat};
use egui_extras::{Size, StripBuilder};
use graphics::graphics::GraphicsBase;
use graphics_backend_traits::traits::GraphicsBackendInterface;
use ui_base::{
    components::menu_top_button::text_icon,
    types::{UIPipe, UIState},
};

use crate::main_menu::user_data::UserData;

/// big box, rounded edges
pub fn render<B: GraphicsBackendInterface>(
    ui: &mut egui::Ui,
    pipe: &mut UIPipe<UserData>,
    ui_state: &mut UIState,
    graphics: &mut GraphicsBase<B>,
    main_frame_only: bool,
) {
    if main_frame_only {
        ui.painter().add(Shape::Rect(RectShape::filled(
            ui.available_rect_before_wrap(),
            5.0,
            ui.style().visuals.window_fill,
        )));
    } else {
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
                                        let mut text = text_icon(ui, "\u{e533}");
                                        text.append(
                                            "Friends",
                                            5.0,
                                            TextFormat::simple(
                                                FontId::default(),
                                                ui.style().visuals.text_color(),
                                            ),
                                        );
                                        ui.label(text);
                                    },
                                );
                            });
                            strip.empty();
                            strip.cell(|ui| {
                                super::table::render(ui, pipe, ui_state, graphics);
                            });
                            strip.empty();
                        });
                });
                strip.empty();
            });
    }
}
