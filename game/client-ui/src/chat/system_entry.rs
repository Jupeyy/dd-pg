use client_types::chat::MsgSystem;
use egui::{
    epaint::Shadow, text::LayoutJob, Align, Color32, FontId, Layout, RichText, Stroke, Vec2,
};
use ui_base::types::{UIPipe, UIState};

use super::user_data::UserData;

/// one chat entry
pub fn render(
    ui: &mut egui::Ui,
    _pipe: &mut UIPipe<UserData>,
    _ui_state: &mut UIState,
    msg: &MsgSystem,
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
