use std::time::Duration;

use egui::FontDefinitions;
use serde::{Deserialize, Serialize};

pub struct UIPipe<'a, U: 'a> {
    pub cur_time: Duration,
    pub user_data: &'a mut U,
}

impl<'a, U: 'a> UIPipe<'a, U> {
    pub fn new(cur_time: Duration, user_data: &'a mut U) -> Self {
        Self {
            cur_time,
            user_data,
        }
    }
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

/// for encode and decode
#[derive(Debug, Serialize, Deserialize)]
pub struct RawInputWrapper {
    pub input: egui::RawInput,
}

/// for encode and decode
#[derive(Serialize, Deserialize)]
pub struct RawOutputWrapper {
    pub output: egui::PlatformOutput,
    pub zoom_level: Option<f32>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UIFonts {
    pub fonts: FontDefinitions,
}
