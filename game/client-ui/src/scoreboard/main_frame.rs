use egui::{Pos2, Rect, Vec2};
use graphics::graphics::GraphicsBase;
use graphics_backend_traits::traits::GraphicsBackendInterface;
use ui_base::types::{UIPipe, UIState};

use super::user_data::UserData;

/// big square, rounded edges
pub fn render<B: GraphicsBackendInterface + 'static>(
    ui: &mut egui::Ui,
    pipe: &mut UIPipe<UserData>,
    ui_state: &mut UIState,
    graphics: &mut GraphicsBase<B>,
    main_frame_only: bool,
) {
    // max possible rect
    let available_rect = ui.available_rect_before_wrap();
    let full_width = available_rect.width();
    let full_height = available_rect.height();

    let allowed_width = full_width.clamp(0.0, 1500.0); // clamp to something sane

    // scoreboard get's 2 / 3 of the space minus some space for the score, a win message etc.
    let offset_y = 50.0;
    let allowed_height_no_spec = (full_height * 2.0 / 3.0) - offset_y;

    // normal centering
    let offset_x = (full_width - allowed_width) / 2.0;

    let no_spec_rect = Rect::from_min_size(
        Pos2::new(offset_x, offset_y),
        Vec2::new(allowed_width, allowed_height_no_spec),
    );

    ui.allocate_ui_at_rect(no_spec_rect, |ui| {
        super::content::main_frame::render(
            ui,
            pipe,
            ui_state,
            graphics,
            main_frame_only,
            available_rect,
        );
    });
}
