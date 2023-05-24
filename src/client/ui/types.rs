use std::sync::Arc;

use base::{config::Config, system};

use graphics::graphics::Graphics;
use native::input::sdl_to_egui::EguiSDL2State;

/**
 * The feedback interface gives the ui the possibility
 * to communicate with the caller/owner of the UI
 */
pub trait UIFeedbackInterface {
    fn network_connect(&mut self, _addr: &str) {
        panic!("this function was not implemented");
    }
    fn network_disconnect(&mut self) {
        panic!("this function was not implemented");
    }
}

pub struct UIPipe<'a> {
    pub ui_feedback: &'a mut dyn UIFeedbackInterface,
    pub graphics: &'a mut Graphics,
    pub sys: &'a system::System,
    pub runtime_thread_pool: &'a Arc<rayon::ThreadPool>,
    pub config: &'a mut Config,
}

pub struct UIState {
    pub sdl2_state: EguiSDL2State,
    pub is_ui_open: bool,

    pub zoom_level: f32,
}

impl UIState {
    pub fn new(zoom_level: f32) -> Self {
        Self {
            sdl2_state: EguiSDL2State::new(zoom_level),

            is_ui_open: true,

            zoom_level: zoom_level,
        }
    }
}
