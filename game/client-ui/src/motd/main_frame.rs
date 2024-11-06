use egui::{vec2, Align2, Frame, Vec2, Window};

use ui_base::types::UiRenderPipe;

use super::user_data::UserData;

/// not required
pub fn render(ui: &mut egui::Ui, pipe: &mut UiRenderPipe<UserData>, main_frame_only: bool) {
    ui.style_mut().animation_time = 0.0;
    ui.set_clip_rect(ui.available_rect_before_wrap());

    Window::new("")
        .resizable(false)
        .title_bar(false)
        .frame(Frame::none())
        .anchor(Align2::CENTER_CENTER, Vec2::new(0.0, 0.0))
        .fixed_size(vec2(400.0, 600.0))
        .show(ui.ctx(), |ui| {
            ui.style_mut().spacing.item_spacing.y = 0.0;
            if main_frame_only {
                // we don't need this
            } else {
                let mut cache = egui_commonmark::CommonMarkCache::default();
                egui_commonmark::CommonMarkViewer::new().show(ui, &mut cache, pipe.user_data.msg);
            }
        });
}
