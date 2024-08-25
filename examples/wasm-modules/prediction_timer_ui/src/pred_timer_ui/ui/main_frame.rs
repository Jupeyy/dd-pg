use std::time::Duration;

use egui::{pos2, vec2, Align2, Color32, FontId, Frame, Rect, Rounding};
use game_interface::{types::render::character::TeeEye, votes::Voted};
use math::math::vector::vec2;
use ui_base::types::{UiRenderPipe, UiState};

use super::user_data::UserData;

/// not required
pub fn render(
    ui: &mut egui::Ui,
    pipe: &mut UiRenderPipe<UserData>,
    ui_state: &mut UiState,
    main_frame_only: bool,
) {
    let prediction_timer = &mut *pipe.user_data.prediction_timer;
    let history = &mut *pipe.user_data.history;
    let props = &mut *pipe.user_data.props;
    let rng = &mut *pipe.user_data.rng;

    super::simulation::simulate(
        history,
        prediction_timer,
        props,
        rng,
        &pipe.cur_time,
        &mut *pipe.user_data.last_time,
    );

    egui::Frame::window(ui.style()).show(ui, |ui| {
        super::plot::render(ui, history, prediction_timer);
        super::settings::render(ui, props);

        ui.label(format!("{:?}", pipe.cur_time));
    });
}
