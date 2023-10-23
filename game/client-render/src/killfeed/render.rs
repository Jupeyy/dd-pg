use std::collections::VecDeque;

use base::system::{self, SystemTimeInterface};
use client_types::killfeed::KillfeedMsg;
use config::config::Config;
use egui::{Color32, Layout};
use graphics_backend::types::Graphics;
use graphics_base_traits::traits::GraphicsSizeQuery;
use ui_base::{
    types::{UIFeedbackInterface, UINativePipe, UIPipe, UIState},
    ui::{UIDummyRawInputGenerator, UIDummyState, UIInterface, UI},
    ui_render::render_ui,
};

pub struct KillfeedRenderPipe<'a> {
    pub graphics: &'a mut Graphics,
    pub sys: &'a system::System,
    pub config: &'a mut Config,
    pub msgs: &'a VecDeque<KillfeedMsg>,
}

pub struct ClientStatsUIFeedbackDummy {}

impl UIFeedbackInterface for ClientStatsUIFeedbackDummy {}

pub struct KillfeedRender {
    ui: UI<UIDummyState>,
}

impl KillfeedRender {
    pub fn new() -> Self {
        let mut ui = UI::new(UIDummyState::default(), None);
        ui.set_main_panel_color(&Color32::TRANSPARENT);
        Self { ui }
    }

    pub fn render_ui(
        msgs: &VecDeque<KillfeedMsg>,
        ui: &mut egui::Ui,
        _pipe: &mut UIPipe<()>,
        _state: &mut UIState,
    ) {
        ui.with_layout(Layout::bottom_up(egui::Align::Min), |ui| {
            for msg in msgs.iter().rev() {
                ui.label(&msg.attacker.name);
            }
        });
    }

    pub fn render(&mut self, pipe: &mut KillfeedRenderPipe) {
        let window_width = pipe.graphics.window_width();
        let window_height = pipe.graphics.window_height();
        let window_pixels_per_point = pipe.graphics.window_pixels_per_point();
        let (screen_rect, full_output, zoom_level) = self.ui.render(
            window_width,
            window_height,
            window_pixels_per_point,
            |ui, inner_pipe, ui_state| Self::render_ui(pipe.msgs, ui, inner_pipe, ui_state),
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
