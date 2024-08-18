use client_types::server_browser::ServerBrowserServer;
use egui::{epaint::RectShape, Color32, FontId, Frame, Layout, RichText, Shape, TextFormat};
use egui_extras::{Size, StripBuilder};

use ui_base::{components::menu_top_button::text_icon, types::UiState};

/// big box, rounded edges
pub fn render(
    ui: &mut egui::Ui,
    ui_state: &mut UiState,
    main_frame_only: bool,
    cur_server: &ServerBrowserServer,
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
                                                            let mut text =
                                                                text_icon(ui, "\u{f05a}");
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
                                                    ui.label(
                                                        RichText::new(format!(
                                                            "Version: {}",
                                                            cur_server.info.version
                                                        ))
                                                        .size(10.0),
                                                    );
                                                    ui.label(
                                                        RichText::new(format!(
                                                            "Game type: {}",
                                                            cur_server.info.game_type
                                                        ))
                                                        .size(10.0),
                                                    );
                                                });
                                            });
                                    });
                                    strip.cell(|ui| {
                                        super::player_list::table::render(ui, ui_state, cur_server);
                                    });
                                    strip.empty();
                                });
                        });
                        strip.empty();
                    });
            });
    }
}
