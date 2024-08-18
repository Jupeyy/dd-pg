use egui::{Color32, Frame, Label, Layout, Sense};
use math::math::vector::vec2;
use ui_base::types::{UiRenderPipe, UiState};

use crate::main_menu::user_data::UserData;

/// single server list entry
pub fn render(
    ui: &mut egui::Ui,
    bg_menu_map: &str,
    pipe: &mut UiRenderPipe<UserData>,
    ui_state: &mut UiState,
) {
    let theme_size = 200.0;
    let entry_size = theme_size + 25.0;
    let (rect, sense) = ui.allocate_exact_size(egui::vec2(entry_size, entry_size), Sense::click());

    let bg_map = &mut pipe.user_data.config.engine.ui.menu_background_map;
    ui.allocate_ui_at_rect(rect, |ui| {
        ui.with_layout(
            Layout::top_down(egui::Align::Center)
                .with_main_justify(true)
                .with_cross_justify(true)
                .with_main_wrap(true),
            |ui| {
                let mut clicked = sense.clicked();
                Frame::default()
                    .fill(if bg_map == bg_menu_map {
                        Color32::from_rgba_unmultiplied(0, 0, 50, 100)
                    } else {
                        Color32::from_rgba_unmultiplied(0, 0, 0, 100)
                    })
                    .rounding(5.0)
                    .show(ui, |ui| {
                        ui.vertical(|ui| {
                            let theme_rect = ui.available_rect_before_wrap();

                            let pos = vec2::new(
                                theme_rect.min.x + entry_size / 2.0,
                                theme_rect.min.y + theme_size / 2.0,
                            );

                            let rect = ui.available_rect_before_wrap();
                            let height_diff = theme_rect.height() - rect.height();
                            let _ =
                                ui.allocate_space(egui::vec2(entry_size, theme_size - height_diff));
                            clicked |= ui
                                .with_layout(
                                    Layout::top_down(egui::Align::Center).with_cross_justify(true),
                                    |ui| ui.add(Label::new(bg_menu_map.to_string()).wrap(true)),
                                )
                                .inner
                                .clicked();
                            ui.add_space(ui.available_height());

                            if ui.is_rect_visible(egui::Rect::from_min_size(
                                theme_rect.min,
                                egui::vec2(entry_size, entry_size),
                            )) {}
                        });
                    });
                if clicked {
                    *bg_map = bg_menu_map.to_string();
                }
            },
        );
    });
}
