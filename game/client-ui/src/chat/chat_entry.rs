use client_types::chat::ChatMsg;
use egui::{
    epaint::Shadow, text::LayoutJob, Align, Color32, FontId, Layout, Rect, RichText, Stroke, Vec2,
};
use math::math::vector::vec2;
use ui_base::types::{UIPipe, UIState};

use crate::utils::render_tee_for_ui;

use super::user_data::UserData;

/// one chat entry
pub fn render(
    ui: &mut egui::Ui,
    pipe: &mut UIPipe<UserData>,
    _ui_state: &mut UIState,
    msg: &ChatMsg,
    full_rect: &Rect,
) {
    let style = ui.style();
    egui::Frame::group(style)
        .fill(style.visuals.window_fill)
        .stroke(Stroke::NONE)
        .shadow(Shadow {
            color: style.visuals.window_shadow.color,
            spread: 5.0,
            ..Default::default()
        })
        .show(ui, |ui| {
            let tee_size = 20.0;
            let margin_from_tee = 5.0;
            let response = ui.horizontal(|ui| {
                ui.add_space(tee_size + margin_from_tee);
                ui.style_mut().spacing.item_spacing.x = 4.0;
                ui.style_mut().spacing.item_spacing.y = 0.0;
                ui.with_layout(Layout::bottom_up(egui::Align::Min), |ui| {
                    ui.add_space(2.0);
                    ui.label(RichText::new(&msg.msg).color(Color32::WHITE));
                    ui.allocate_ui_with_layout(
                        Vec2::new(ui.available_width(), 12.0),
                        Layout::left_to_right(Align::Max),
                        |ui| {
                            let text_format = egui::TextFormat {
                                line_height: Some(10.0),
                                font_id: FontId::proportional(10.0),
                                valign: Align::BOTTOM,
                                color: Color32::WHITE,
                                ..Default::default()
                            };
                            let mut job =
                                LayoutJob::single_section(msg.player.clone(), text_format);
                            let text_format_date = egui::TextFormat {
                                line_height: Some(10.0),
                                font_id: FontId::proportional(8.0),
                                valign: Align::BOTTOM,
                                color: Color32::LIGHT_GRAY,
                                ..Default::default()
                            };
                            job.append(&msg.clan, 4.0, text_format_date);
                            ui.label(job);
                        },
                    );
                    ui.add_space(2.0);
                });
                ui.add_space(ui.available_width().min(4.0));
            });

            let rect = response.response.rect;

            render_tee_for_ui(
                pipe.user_data.canvas_handle,
                ui,
                *full_rect,
                Some(ui.clip_rect()),
                &msg.skin_name,
                vec2::new(
                    rect.min.x + tee_size / 2.0,
                    rect.min.y + tee_size / 2.0 + 5.0,
                ),
                tee_size,
            );
        });
}
