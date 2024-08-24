use std::{collections::VecDeque, thread::ThreadId, time::Duration};

use api::{graphics::graphics::GraphicsBackend, println, GRAPHICS, IO, RUNTIME_THREAD_POOL};
use api_ui_game::render::{create_emoticons_container, create_skin_container};
use base_io::{io_batcher::IoBatcherTask, yield_now::yield_now};
use client_containers::{emoticons::EmoticonsContainer, skins::SkinContainer};
use client_render_base::render::tee::RenderTee;
use client_types::{
    console::ConsoleEntry,
    server_browser::{ServerBrowserData, ServerBrowserFilter, ServerBrowserServer},
};
use client_ui::console::user_data::UserData;
use game_interface::types::character_info::NetworkSkinInfo;
use graphics::{
    graphics::graphics::Graphics,
    handles::{canvas::canvas::GraphicsCanvasHandle, stream::stream::GraphicsStreamHandle},
};
use math::math::{vector::ubvec4, Rng};
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

impl PredTimerPage {
    pub fn new(graphics: &Graphics) -> Self {
        Self {
            prediction_timer: PredictionTimer::new(Duration::ZERO, Duration::ZERO),
            history: Default::default(),
            rng: Rng::new(0),
            props: SimulationProps::default(),
            last_time: Duration::ZERO,
        }
    }

    fn render_impl(
        &mut self,
        ui: &mut egui::Ui,
        pipe: &mut UiRenderPipe<()>,
        ui_state: &mut UiState,
        main_frame_only: bool,
    ) {
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
            ui_state,
            main_frame_only,
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
        ui_state: &mut UiState,
    ) {
        self.render_impl(ui, pipe, ui_state, true)
    }

    fn render(&mut self, ui: &mut egui::Ui, pipe: &mut UiRenderPipe<()>, ui_state: &mut UiState) {
        self.render_impl(ui, pipe, ui_state, false)
    }
}
