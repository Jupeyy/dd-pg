use std::sync::Arc;

use base::system;

use config::config::Config;

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
    fn call_path(&mut self, _config: &mut Config, _mod_name: &str, _path: &str) {
        panic!("this function was not implemented");
    }
}

pub trait UIRawInputGenerator<T> {
    fn get_raw_input(&self, state: &mut UIState<T>) -> egui::RawInput;
}

pub struct UIPipe<'a, T> {
    pub ui_feedback: &'a mut dyn UIFeedbackInterface,
    pub sys: &'a system::System,
    pub runtime_thread_pool: &'a Arc<rayon::ThreadPool>,
    pub config: &'a mut Config,
    pub raw_inp_generator: &'a dyn UIRawInputGenerator<T>,
}

pub struct UIState<T> {
    pub native_state: T,
    pub is_ui_open: bool,

    pub zoom_level: f32,
}

impl<T> UIState<T> {
    pub fn new(state: T, zoom_level: f32) -> Self {
        Self {
            native_state: state,

            is_ui_open: true,

            zoom_level: zoom_level,
        }
    }
}
