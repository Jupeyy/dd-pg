use std::{sync::Arc, time::Duration};

use arrayvec::ArrayString;
use config::config::Config;
use egui::Color32;
use egui_extras::StripBuilder;
use fixed::{types::extra::U16, FixedI64};
use graphics::graphics::Graphics;
use graphics_traits::GraphicsSizeQuery;
use shared_network::messages::ServerToClientMessage;
use ui_wasm_manager::ui_render::{destroy_ui, render_ui};

use crate::client::component::{
    ComponentGameMsg, ComponentLoadIOPipe, ComponentLoadPipe, ComponentLoadWhileIOPipe,
    ComponentLoadable, ComponentUpdatable, ComponentUpdatePipe,
};

use ui_base::{
    types::{UIFeedbackInterface, UIPipe, UIState},
    ui::{UIDummyRawInputGenerator, UIDummyState, UIInterface, UI},
};

use math::math::blend;

use base::system::{self, SystemTimeInterface};

pub struct ClientStatsUIFeedbackDummy {}

impl UIFeedbackInterface for ClientStatsUIFeedbackDummy {}

pub struct ClientStatsData {
    last_frame_time: Duration,
    fps: FixedI64<U16>,
}

impl ClientStatsData {
    pub fn new(sys: &system::System) -> Self {
        Self {
            fps: FixedI64::from_num(60.0),
            last_frame_time: sys.time_get_nanoseconds(),
        }
    }

    pub fn render_stats(
        &mut self,
        ui: &mut egui::Ui,
        pipe: &mut UIPipe<UIDummyState>,
        _ui_state: &mut UIState<UIDummyState>,
    ) {
        let cur_time = pipe.sys.time_get_nanoseconds();
        let time_diff = cur_time - self.last_frame_time;
        self.last_frame_time = cur_time;

        self.fps = blend(
            &self.fps,
            &(FixedI64::from_num(Duration::from_secs(1).as_nanos())
                / FixedI64::from_num(time_diff.as_nanos())),
            FixedI64::from_num(1),
            FixedI64::from_num(0.1),
        );

        StripBuilder::new(ui)
            .size(egui_extras::Size::remainder())
            .size(egui_extras::Size::exact(60.0))
            .horizontal(|mut strip| {
                strip.cell(|_| {});
                strip.cell(|ui| {
                    ui.colored_label(
                        Color32::from_rgb(255, 0, 255),
                        format!("{}", self.fps.floor()),
                    );
                })
            });
    }
}

pub struct ClientStatsRenderPipe<'a> {
    pub graphics: &'a mut Graphics,
    pub sys: &'a system::System,
    pub runtime_thread_pool: &'a Arc<rayon::ThreadPool>,
    pub config: &'a mut Config,
}

/**
 * This component collects various client statistics and displays them optionally
 */
pub struct ClientStats {
    data: ClientStatsData,
    ui: UI<UIDummyState>,
}

impl ClientStats {
    pub fn new(sys: &system::System) -> Self {
        let mut ui = UI::new(UIDummyState::default(), 2.5);
        ui.set_main_panel_color(&Color32::TRANSPARENT);
        Self {
            data: ClientStatsData::new(sys),
            ui,
        }
    }

    pub fn render(&mut self, pipe: &mut ClientStatsRenderPipe) {
        let canvas_width = pipe.graphics.canvas_width();
        let canvas_height = pipe.graphics.canvas_height();
        let (screen_rect, full_output) = self.ui.render(
            canvas_width,
            canvas_height,
            |ui, pipe, ui_state| self.data.render_stats(ui, pipe, ui_state),
            &mut UIPipe {
                ui_feedback: &mut ClientStatsUIFeedbackDummy {},
                sys: pipe.sys,
                runtime_thread_pool: pipe.runtime_thread_pool,
                config: pipe.config,
                raw_inp_generator: &UIDummyRawInputGenerator {},
            },
        );
        render_ui(&mut self.ui, full_output, &screen_rect, &mut pipe.graphics);
    }
}

impl ComponentLoadable for ClientStats {
    fn load_io(&mut self, _io_pipe: &mut ComponentLoadIOPipe) {}

    fn init_while_io(&mut self, _pipe: &mut ComponentLoadWhileIOPipe) {}

    fn init(&mut self, _pipe: &mut ComponentLoadPipe) -> Result<(), ArrayString<4096>> {
        Ok(())
    }

    fn destroy(self, pipe: &mut crate::client::component::ComponentDestroyPipe) {
        destroy_ui(self.ui, pipe.graphics);
    }
}

impl ComponentUpdatable for ClientStats {
    fn update(&mut self, _pipe: &mut ComponentUpdatePipe) {
        // TODO: some CPU frametime stats or smth similar
    }
}

impl ComponentGameMsg for ClientStats {
    // TODO: network stats
    fn on_msg(
        &mut self,
        _timestamp: &std::time::Duration,
        _msg: ServerToClientMessage,
        _pipe: &mut crate::client::component::GameMsgPipeline,
    ) {
    }

    fn on_connect(&mut self, _timestamp: &std::time::Duration) {}

    fn on_disconnect(&mut self, _timestamp: &std::time::Duration) {}
}
