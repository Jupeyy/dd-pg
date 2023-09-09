use std::time::Duration;

use bincode::{BorrowDecode, Decode, Encode};
use config::config::Config;
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
    fn get_raw_input(&self, state: &mut UIState<T>) -> egui::RawInput;
}

pub struct UIPipe<'a, T> {
    pub ui_feedback: &'a mut dyn UIFeedbackInterface,
    pub cur_time: Duration,
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

            zoom_level,
        }
    }
}

/// for encode and decode
#[derive(Debug, Serialize, Deserialize)]
pub struct RawInputWrapper {
    pub input: egui::RawInput,
}

impl Encode for RawInputWrapper {
    fn encode<E: bincode::enc::Encoder>(
        &self,
        encoder: &mut E,
    ) -> Result<(), bincode::error::EncodeError> {
        let conf = *encoder.config();
        bincode::serde::encode_into_writer(self, encoder.writer(), conf)
    }
}

impl Decode for RawInputWrapper {
    fn decode<D: bincode::de::Decoder>(
        decoder: &mut D,
    ) -> Result<Self, bincode::error::DecodeError> {
        let conf = *decoder.config();
        bincode::serde::decode_from_reader(decoder.reader(), conf)
    }
}

impl<'de> BorrowDecode<'de> for RawInputWrapper {
    fn borrow_decode<D: bincode::de::BorrowDecoder<'de>>(
        decoder: &mut D,
    ) -> Result<Self, bincode::error::DecodeError> {
        Self::decode(decoder)
    }
}
