use egui::{epaint::RectShape, Color32, FontId, Layout, RichText, Shape, TextFormat};
use egui_extras::{Size, StripBuilder};
use graphics::graphics::Graphics;
use ui_base::{
    components::menu_top_button::text_icon,
    types::{UIPipe, UIState},
};

use crate::main_menu::user_data::UserData;

/// big box, rounded edges
pub fn render(
    ui: &mut egui::Ui,
    pipe: &mut UIPipe<UserData>,
    ui_state: &mut UIState,
    graphics: &mut Graphics,
    main_frame_only: bool,
) {
    if main_frame_only {
        ui.painter().add(Shape::Rect(RectShape::filled(
            ui.available_rect_before_wrap(),
            5.0,
            Color32::from_rgba_unmultiplied(0, 0, 0, 255),
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
                    let server_details_height = 70.0;
                    StripBuilder::new(ui)
                        .size(Size::exact(0.0))
                        .size(Size::exact(server_details_height))
                        .size(Size::remainder())
                        .size(Size::exact(item_spacing))
                        .clip(true)
                        .vertical(|mut strip| {
                            strip.empty();
                            strip.cell(|ui| {
                                StripBuilder::new(ui)
                                    .size(Size::exact(30.0))
                                    .size(Size::remainder())
                                    .vertical(|mut strip| {
                                        strip.cell(|ui| {
                                            ui.with_layout(
                                                Layout::left_to_right(egui::Align::Center)
                                                    .with_main_align(egui::Align::Center)
                                                    .with_main_justify(true),
                                                |ui| {
                                                    let mut text = text_icon(ui, "\u{f05a}");
                                                    text.append(
                                                        "Server details",
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
                                        strip.cell(|ui| {
                                            ui.label(RichText::new("Version: pg").size(10.0));
                                            ui.label(RichText::new("Game type: my_mod").size(10.0));
                                        });
                                    });
                            });
                            strip.cell(|ui| {
                                super::player_list::table::render(ui, pipe, ui_state, graphics);
                            });
                            strip.empty();
                        });
                });
                strip.empty();
            });
    }
}
