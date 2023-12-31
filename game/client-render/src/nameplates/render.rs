use base::system::{self, SystemTimeInterface};
use config::config::ConfigEngine;
use egui::Color32;
use graphics::graphics::Graphics;

use ui_base::{
    types::{UIFeedbackInterface, UINativePipe, UIPipe},
    ui::{UIDummyRawInputGenerator, UIDummyState, UI},
    ui_render::render_ui,
};

pub struct NameplateRenderPipe<'a> {
    pub sys: &'a system::System,
    pub config: &'a mut ConfigEngine,
    pub graphics: &'a mut Graphics,
    pub name: &'a str,
}

pub struct ClientStatsUIFeedbackDummy {}

impl UIFeedbackInterface for ClientStatsUIFeedbackDummy {}

pub struct NameplateRender {
    ui: UI<UIDummyState>,
}

impl NameplateRender {
    pub fn new() -> Self {
        let mut ui = UI::new(Default::default(), None);
        ui.set_main_panel_color(&Color32::TRANSPARENT);
        Self { ui }
    }

    pub fn render(&mut self, pipe: &mut NameplateRenderPipe) {
        let window_width = pipe.graphics.canvas_handle.window_width();
        let window_height = pipe.graphics.canvas_handle.window_height();
        let window_pixels_per_point = pipe.graphics.canvas_handle.window_pixels_per_point();

        let mut ui_feedback = ClientStatsUIFeedbackDummy {};
        let mut dummy_pipe = UIPipe::new(
            &mut ui_feedback,
            pipe.sys.time_get_nanoseconds(),
            pipe.config,
            (),
        );
        let mut dummy_native_pipe = UINativePipe {
            raw_inp_generator: &UIDummyRawInputGenerator {},
        };
        let (screen_rect, full_output, zoom_level) = self.ui.render(
            window_width,
            window_height,
            window_pixels_per_point,
            |ui, _inner_pipe, _ui_state| {
                ui.label(pipe.name);
            },
            &mut dummy_pipe,
            &mut dummy_native_pipe,
            false,
        );
        render_ui(
            &mut self.ui,
            &mut dummy_native_pipe,
            full_output,
            &screen_rect,
            zoom_level,
            &mut pipe.graphics,
            false,
        );
    }
}
