use std::{collections::VecDeque, time::Duration};

use math::math::Rng;
use prediction_timer::prediction_timing::{PredictionTimer, PredictionTiming};
use ui_base::types::{UiRenderPipe, UiState};
use ui_traits::traits::UiPageInterface;

use super::ui::user_data::SimulationProps;

pub struct PredTimerPage {
    prediction_timer: PredictionTimer,
    history: VecDeque<PredictionTiming>,
    rng: Rng,
    props: SimulationProps,
    last_time: Duration,
}

impl Default for PredTimerPage {
    fn default() -> Self {
        Self {
            prediction_timer: PredictionTimer::new(Duration::ZERO, Duration::ZERO),
            history: Default::default(),
            rng: Rng::new(0),
            props: SimulationProps::default(),
            last_time: Duration::ZERO,
        }
    }
}

impl PredTimerPage {
    pub fn new() -> Self {
        Self::default()
    }

    fn render_impl(&mut self, ui: &mut egui::Ui, pipe: &mut UiRenderPipe<()>) {
        super::ui::main_frame::render(
            ui,
            &mut UiRenderPipe::new(
                pipe.cur_time,
                &mut super::ui::user_data::UserData {
                    prediction_timer: &mut self.prediction_timer,
                    history: &mut self.history,
                    props: &mut self.props,
                    rng: &mut self.rng,
                    last_time: &mut self.last_time,
                },
            ),
        );
    }
}

impl UiPageInterface<()> for PredTimerPage {
    fn has_blur(&self) -> bool {
        false
    }

    fn render_main_frame(
        &mut self,
        ui: &mut egui::Ui,
        pipe: &mut UiRenderPipe<()>,
        _ui_state: &mut UiState,
    ) {
        self.render_impl(ui, pipe)
    }

    fn render(&mut self, ui: &mut egui::Ui, pipe: &mut UiRenderPipe<()>, _ui_state: &mut UiState) {
        self.render_impl(ui, pipe)
    }
}
