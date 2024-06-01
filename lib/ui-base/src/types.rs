use std::{collections::HashMap, rc::Rc, sync::Arc, time::Duration};

use egui::{FontDefinitions, Rect};
use serde::{Deserialize, Serialize};

use crate::custom_callback::CustomCallbackTrait;

pub struct UiRenderPipe<'a, U: 'a> {
    pub cur_time: Duration,
    pub user_data: &'a mut U,
}

impl<'a, U: 'a> UiRenderPipe<'a, U> {
    pub fn new(cur_time: Duration, user_data: &'a mut U) -> Self {
        Self {
            cur_time,
            user_data,
        }
    }
}

#[derive(Debug)]
pub struct UiState {
    pub is_ui_open: bool,
    pub hint_had_input: bool,

    pub zoom_level: Option<f32>,

    pub custom_paints: HashMap<u64, Rc<dyn CustomCallbackTrait>>,
    pub custom_paint_id: u64,
}

impl UiState {
    /// zoom level is optional. and means that it overrides the default value
    pub fn new(zoom_level: Option<f32>) -> Self {
        Self {
            is_ui_open: true,
            hint_had_input: false,

            zoom_level,

            custom_paints: Default::default(),
            custom_paint_id: 0,
        }
    }

    pub fn add_custom_paint(
        &mut self,
        ui: &mut egui::Ui,
        render_rect: Rect,
        custom_paint: Rc<dyn CustomCallbackTrait>,
    ) {
        let id = self.custom_paint_id;
        self.custom_paint_id += 1;
        self.custom_paints.insert(id, custom_paint);
        ui.painter().add(egui::PaintCallback {
            rect: render_rect,
            callback: Arc::new(id),
        });
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
pub struct UiFonts {
    pub fonts: FontDefinitions,
}
