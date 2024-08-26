use egui::{Pos2, Rect, Vec2};

use ui_base::types::{UiRenderPipe, UiState};

use super::user_data::UserData;

/// not required
pub fn render(
    ui: &mut egui::Ui,
    pipe: &mut UiRenderPipe<UserData>,
    ui_state: &mut UiState,
    main_frame_only: bool,
) {
    let margin = 15.0;
    let x_offset = margin;

    let width = (ui.available_width() / 2.0).min(500.0) - x_offset;
    let (y_offset, height) = if !pipe.user_data.show_chat_history {
        (
            // chat renders in the lower 1/3 of the ui height
            ui.available_height() * 2.0 / 3.0,
            (ui.available_height() * 1.0 / 3.0) - margin,
        )
    } else {
        (
            // chat renders in the lower 2/3 of the ui height
            ui.available_height() * 1.0 / 3.0,
            (ui.available_height() * 2.0 / 3.0) - margin,
        )
    };

    let render_rect = Rect::from_min_size(Pos2::new(x_offset, y_offset), Vec2::new(width, height));

    let full_rect = ui.available_rect_before_wrap();

    ui.allocate_ui_at_rect(render_rect, |ui| {
        ui.set_clip_rect(ui.available_rect_before_wrap());
        if main_frame_only {
            // we don't need this
        } else {
            super::chat_list::render(ui, pipe, ui_state, &full_rect)
        }
    });
}
