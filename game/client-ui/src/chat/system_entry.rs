use client_types::chat::MsgSystem;
use egui::{text::LayoutJob, Align, Color32, FontId, Layout, RichText, Vec2};
use ui_base::types::{UiRenderPipe, UiState};

use super::{shared::entry_frame, user_data::UserData};

/// one chat entry
pub fn render(
    ui: &mut egui::Ui,
    _pipe: &mut UiRenderPipe<UserData>,
    _ui_state: &mut UiState,
    msg: &MsgSystem,
) {
    entry_frame(ui, |ui| {
        ui.horizontal(|ui| {
            ui.style_mut().spacing.item_spacing.x = 4.0;
            ui.style_mut().spacing.item_spacing.y = 0.0;
            ui.with_layout(Layout::bottom_up(egui::Align::Min), |ui| {
                let color = Color32::from_rgba_unmultiplied(255, 238, 0, 255);
                ui.add_space(2.0);
                ui.label(RichText::new(&msg.msg).color(color));
                ui.allocate_ui_with_layout(
                    Vec2::new(ui.available_width(), 12.0),
                    Layout::left_to_right(Align::Max),
                    |ui| {
                        let text_format = egui::TextFormat {
                            line_height: Some(10.0),
                            font_id: FontId::proportional(10.0),
                            valign: Align::BOTTOM,
                            color,
                            ..Default::default()
                        };
                        let job = LayoutJob::single_section("System".to_string(), text_format);
                        ui.label(job);
                    },
                );
                ui.add_space(2.0);
            });
            ui.add_space(ui.available_width().min(4.0));
        });
    });
}
