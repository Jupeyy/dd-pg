use base::system::{self, SystemTimeInterface};
use config::config::Config;
use egui::Color32;
use graphics_backend::types::Graphics;
use graphics_base_traits::traits::GraphicsSizeQuery;
use ui_base::{
    types::{UIFeedbackInterface, UINativePipe, UIPipe, UIState},
    ui::{UIDummyRawInputGenerator, UIDummyState, UIInterface, UI},
    ui_render::render_ui,
};

use super::types::ScoreboardEntry;

pub struct ScoreboardRenderPipe<'a> {
    pub graphics: &'a mut Graphics,
    pub sys: &'a system::System,
    pub config: &'a mut Config,
    pub entries: &'a Vec<ScoreboardEntry>,
}

pub struct ClientStatsUIFeedbackDummy {}

impl UIFeedbackInterface for ClientStatsUIFeedbackDummy {}

pub struct ScoreboardRender {
    ui: UI<UIDummyState>,
}

impl ScoreboardRender {
    pub fn new() -> Self {
        let mut ui = UI::new(UIDummyState::default(), None);
        ui.set_main_panel_color(&Color32::TRANSPARENT);
        Self { ui }
    }

    pub fn render_ui(
        entries: &Vec<ScoreboardEntry>,
        ui: &mut egui::Ui,
        _pipe: &mut UIPipe<()>,
        _state: &mut UIState,
    ) {
        ui.horizontal(|ui| {
            for entry in entries {
                ui.label(&entry.name);
            }
        });
    }

    pub fn render(&mut self, pipe: &mut ScoreboardRenderPipe) {
        let window_width = pipe.graphics.window_width();
        let window_height = pipe.graphics.window_height();
        let window_pixels_per_point = pipe.graphics.window_pixels_per_point();
        let (screen_rect, full_output, zoom_level) = self.ui.render(
            window_width,
            window_height,
            window_pixels_per_point,
            |ui, inner_pipe, ui_state| Self::render_ui(pipe.entries, ui, inner_pipe, ui_state),
            &mut UIPipe::new(
                &mut ClientStatsUIFeedbackDummy {},
                pipe.sys.time_get_nanoseconds(),
                pipe.config,
                (),
            ),
            &mut UINativePipe {
                raw_inp_generator: &UIDummyRawInputGenerator {},
            },
            false,
        );
        render_ui(
            &mut self.ui,
            &mut UINativePipe {
                raw_inp_generator: &UIDummyRawInputGenerator {},
            },
            full_output,
            &screen_rect,
            zoom_level,
            &mut pipe.graphics,
            false,
        );
    }
}
