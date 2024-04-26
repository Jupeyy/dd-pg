use egui::{Pos2, Rect, Vec2};

use ui_base::types::{UIPipe, UIState};

use super::user_data::UserData;

/// not required
pub fn render(
    ui: &mut egui::Ui,
    pipe: &mut UIPipe<UserData>,
    ui_state: &mut UIState,
    main_frame_only: bool,
) {
    // last 1/3 is for action feed
    let x_offset = ui.available_width() * 2.0 / 3.0;
    let y_offset = 0.0;

    let width = (ui.available_width() * 1.0 / 3.0).min(500.0);
    let height = ui.available_height() / 2.0;

    let render_rect = Rect::from_min_size(Pos2::new(x_offset, y_offset), Vec2::new(width, height));

    let full_rect = ui.available_rect_before_wrap();

    ui.allocate_ui_at_rect(render_rect, |ui| {
        ui.set_clip_rect(ui.available_rect_before_wrap());
        if main_frame_only {
            // we don't need this
        } else {
            super::feed_list::render(ui, pipe, ui_state, &full_rect)
        }
    });
}
