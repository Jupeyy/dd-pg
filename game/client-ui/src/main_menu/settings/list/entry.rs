use client_containers::container::ContainerItemIndexType;
use egui::{Color32, FontId, Frame, Label, Layout, RichText, Sense, UiBuilder};
use math::math::vector::vec2;

/// single list entry
pub fn render(
    ui: &mut egui::Ui,
    entry_index: usize,
    entry_name: &str,
    ty: ContainerItemIndexType,
    entry_visual_size: f32,
    validation_fn: &impl Fn(usize, &str) -> anyhow::Result<()>,
    is_selected_fn: &impl Fn(usize, &str) -> bool,
    render_fn: &mut impl FnMut(&mut egui::Ui, usize, &str, vec2, f32),
    on_click_fn: &mut impl FnMut(usize, &str),
) {
    let entry_valid = validation_fn(entry_index, entry_name);

    let entry_size = entry_visual_size + 25.0;
    let (rect, sense) = ui.allocate_exact_size(egui::vec2(entry_size, entry_size), Sense::click());

    ui.allocate_new_ui(UiBuilder::new().max_rect(rect), |ui| {
        ui.with_layout(
            Layout::top_down(egui::Align::Center)
                .with_main_justify(true)
                .with_cross_justify(true)
                .with_main_wrap(true),
            |ui| {
                let mut clicked = sense.clicked();
                Frame::default()
                    .fill(if is_selected_fn(entry_index, entry_name) {
                        Color32::from_rgba_unmultiplied(0, 0, 50, 100)
                    } else {
                        Color32::from_rgba_unmultiplied(0, 0, 0, 100)
                    })
                    .rounding(5.0)
                    .show(ui, |ui| {
                        ui.vertical(|ui| {
                            let visual_rect = ui.available_rect_before_wrap();
                            if matches!(ty, ContainerItemIndexType::Http) {
                                ui.painter().text(
                                    visual_rect.min + egui::vec2(5.0, 5.0),
                                    egui::Align2::LEFT_TOP,
                                    "\u{f019}",
                                    FontId::new(12.0, egui::FontFamily::Name("icons".into())),
                                    Color32::WHITE,
                                );
                            }

                            let pos = vec2::new(
                                visual_rect.min.x + entry_size / 2.0,
                                visual_rect.min.y + entry_visual_size / 2.0,
                            );

                            if let Err(err) = &entry_valid {
                                ui.label(RichText::new(err.to_string()).color(Color32::RED));
                            }

                            let rect = ui.available_rect_before_wrap();
                            let height_diff =
                                (visual_rect.height() - rect.height()).clamp(0.0, f32::MAX);
                            let _ = ui.allocate_space(egui::vec2(
                                entry_size,
                                (entry_visual_size - height_diff).clamp(1.0, f32::MAX),
                            ));
                            clicked |= ui
                                .with_layout(
                                    Layout::top_down(egui::Align::Center).with_cross_justify(true),
                                    |ui| ui.add(Label::new(entry_name).wrap()),
                                )
                                .inner
                                .clicked();
                            ui.add_space(ui.available_height());

                            if entry_valid.is_ok()
                                && ui.is_rect_visible(egui::Rect::from_min_size(
                                    visual_rect.min,
                                    egui::vec2(entry_size, entry_size),
                                ))
                            {
                                render_fn(ui, entry_index, entry_name, pos, entry_visual_size);
                            }
                        });
                    });
                if clicked {
                    on_click_fn(entry_index, entry_name);
                }
            },
        );
    });
}
