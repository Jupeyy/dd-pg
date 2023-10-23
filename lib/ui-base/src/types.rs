use std::{marker::PhantomData, time::Duration};

use bincode::{Decode, Encode};
use config::config::Config;
use egui::FontDefinitions;
use serde::{Deserialize, Serialize};

/**
 * The feedback interface gives the ui the possibility
 * to communicate with the caller/owner of the UI
 */
pub trait UIFeedbackInterface {
    fn network_connect(&mut self, _addr: &str) {
        panic!("this function was not implemented");
    }
    fn network_connect_local_player(&mut self) {
        panic!("this function was not implemented");
    }
    fn network_disconnect_local_player(&mut self) {
        panic!("this function was not implemented");
    }
    fn network_disconnect(&mut self) {
        panic!("this function was not implemented");
    }
    fn local_player_count(&self) -> usize {
        panic!("this function was not implemented")
    }
    fn queue_info(&self) -> &str {
        panic!("this function was not implemented")
    }
    fn network_err(&self) -> &str {
        panic!("this function was not implemented")
    }
    fn call_path(&mut self, _config: &mut Config, _mod_name: &str, _path: &str) {
        panic!("this function was not implemented");
    }
}

pub trait UIRawInputGenerator<T> {
    fn get_raw_input(&self, state: &mut UINativeState<T>) -> egui::RawInput;
    fn process_output(
        &self,
        state: &mut UINativeState<T>,
        ctx: &egui::Context,
        output: egui::PlatformOutput,
    );
}

pub struct UIPipe<'a, U> {
    pub ui_feedback: &'a mut dyn UIFeedbackInterface,
    pub cur_time: Duration,
    pub config: &'a mut Config,
    pub user_data: U,
}

impl<'a, U> UIPipe<'a, U> {
    pub fn new(
        ui_feedback: &'a mut dyn UIFeedbackInterface,
        cur_time: Duration,
        config: &'a mut Config,
        user_data: U,
    ) -> Self {
        Self {
            ui_feedback,
            cur_time,
            config,
            user_data,
        }
    }
}

pub struct UINativePipe<'a, T> {
    pub raw_inp_generator: &'a dyn UIRawInputGenerator<T>,
}

pub struct UIState {
    pub is_ui_open: bool,
    pub hint_had_input: bool,

    pub zoom_level: Option<f32>,
}

impl UIState {
    /// zoom level is optional. and means that it overrides the default value
    pub fn new(zoom_level: Option<f32>) -> Self {
        Self {
            is_ui_open: true,
            hint_had_input: false,

            zoom_level,
        }
    }
}

pub struct UINativeState<T> {
    pub native_state: T,
}

impl<T> UINativeState<T> {
    pub fn new(state: T) -> Self {
        Self {
            native_state: state,
        }
    }
}

/// for encode and decode
#[derive(Debug, Serialize, Deserialize, Encode, Decode)]
pub struct RawInputWrapper {
    #[bincode(with_serde)]
    pub input: egui::RawInput,
}

/// for encode and decode
#[derive(Serialize, Deserialize, Encode, Decode)]
pub struct RawOutputWrapper {
    #[bincode(with_serde)]
    pub output: Option<egui::PlatformOutput>,
    pub zoom_level: f32,
}

#[derive(Debug, Serialize, Deserialize, Encode, Decode)]
pub struct UIFonts {
    #[bincode(with_serde)]
    pub fonts: FontDefinitions,
}
