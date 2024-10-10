use egui::{epaint::RectShape, Color32, Frame, Grid, Layout, Rect, RichText, Shape};
use egui_extras::{Size, StripBuilder};
use shared_base::server_browser::ServerBrowserServer;

use ui_base::{
    types::{UiRenderPipe, UiState},
    utils::icon_font_plus_text,
};

use super::player_list::list::entry::EntryData;

/// big box, rounded edges
pub fn render(
    ui: &mut egui::Ui,
    full_rect: &Rect,
    pipe: &mut UiRenderPipe<EntryData>,
    ui_state: &mut UiState,
    main_frame_only: bool,
    cur_server: Option<&ServerBrowserServer>,
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
                                                            ui.label(icon_font_plus_text(
                                                                ui,
                                                                "\u{f05a}",
                                                                "Server details",
                                                            ));
                                                        },
                                                    );
                                                });
                                                strip.cell(|ui| {
                                                    if let Some(cur_server) = cur_server {
                                                        Grid::new("server-details-short")
                                                            .num_columns(2)
                                                            .show(ui, |ui| {
                                                                ui.label(
                                                                    RichText::new("Version:")
                                                                        .size(10.0),
                                                                );
                                                                ui.label(
                                                                    RichText::new(
                                                                        &cur_server.info.version,
                                                                    )
                                                                    .size(10.0),
                                                                );
                                                                ui.end_row();
                                                                ui.label(
                                                                    RichText::new("Game type:")
                                                                        .size(10.0),
                                                                );
                                                                ui.label(
                                                                    RichText::new(
                                                                        &cur_server.info.game_type,
                                                                    )
                                                                    .size(10.0),
                                                                );
                                                                ui.end_row();
                                                            });
                                                    } else {
                                                        ui.label("No server selected");
                                                    }
                                                });
                                            });
                                    });
                                    strip.cell(|ui| {
                                        if let Some(cur_server) = cur_server {
                                            super::player_list::table::render(
                                                ui, full_rect, pipe, ui_state, cur_server,
                                            );
                                        }
                                    });
                                    strip.empty();
                                });
                        });
                        strip.empty();
                    });
            });
    }
}
