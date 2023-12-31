use client_types::chat::ChatMsg;
use egui::{text::LayoutJob, Align, Color32, FontId, Layout, Rect, RichText, Vec2};
use graphics::graphics::Graphics;
use math::math::vector::{vec2, vec4};
use ui_base::types::{UIPipe, UIState};

use super::{
    user_data::UserData,
    utils::{render_rect_afterwards, render_tee_for_chat},
};

/// one chat entry
pub fn render(
    ui: &mut egui::Ui,
    _pipe: &mut UIPipe<UserData>,
    _ui_state: &mut UIState,
    _graphics: &mut Graphics,
    msg: &ChatMsg,
    full_rect: &Rect,
) {
    let after_rect = render_rect_afterwards(
        ui,
        *full_rect,
        Some(ui.clip_rect()),
        vec4::new(0.0, 0.0, 0.0, 0.4),
    );

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
                    let mut job = LayoutJob::single_section(msg.player.clone(), text_format);
                    let text_format_date = egui::TextFormat {
                        line_height: Some(10.0),
                        font_id: FontId::proportional(8.0),
                        valign: Align::BOTTOM,
                        color: Color32::LIGHT_GRAY,
                        ..Default::default()
                    };
                    job.append(&msg.skin_name, 4.0, text_format_date);
                    ui.label(job);
                },
            );
            ui.add_space(2.0);
        });
        ui.add_space(ui.available_width().min(4.0));
    });

    let rect = response.response.rect;

    render_tee_for_chat(
        ui,
        *full_rect,
        Some(ui.clip_rect()),
        vec2::new(
            rect.min.x + tee_size / 2.0,
            rect.min.y + tee_size / 2.0 + 5.0,
        ),
        tee_size,
    );

    after_rect.x.store(
        (rect.min.x as f64 * 1000000000.0) as u64,
        std::sync::atomic::Ordering::SeqCst,
    );
    after_rect.y.store(
        (rect.min.y as f64 * 1000000000.0) as u64,
        std::sync::atomic::Ordering::SeqCst,
    );
    after_rect.w.store(
        (rect.width() as f64 * 1000000000.0) as u64,
        std::sync::atomic::Ordering::SeqCst,
    );
    after_rect.h.store(
        (rect.height() as f64 * 1000000000.0) as u64,
        std::sync::atomic::Ordering::SeqCst,
    );
}
