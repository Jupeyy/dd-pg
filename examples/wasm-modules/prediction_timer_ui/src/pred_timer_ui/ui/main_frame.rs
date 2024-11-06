use ui_base::types::UiRenderPipe;

use super::user_data::UserData;

/// not required
pub fn render(ui: &mut egui::Ui, pipe: &mut UiRenderPipe<UserData>) {
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
        super::plot::render(ui, history);
        super::settings::render(ui, props);

        ui.label(format!(
            "smooth max_ping: {}",
            prediction_timer.snapshot().smooth_max_ping * 1000.0
        ));

        ui.horizontal(|ui| {
            ui.label(format!(
                "{}",
                prediction_timer.snapshot().calc_farsight_of_jitter()
            ));
            ui.label(format!("{:.2?}", pipe.cur_time));
            if ui.button("clear").clicked() {
                history.clear();
            }
        });
    });
}
