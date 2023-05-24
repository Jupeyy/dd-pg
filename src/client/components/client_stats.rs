use std::time::Duration;

use arrayvec::ArrayString;
use egui::Color32;
use egui_extras::StripBuilder;
use fixed::{types::extra::U16, FixedI64};

use crate::{
    client::component::{
        ComponentComponent, ComponentGameMsg, ComponentLoadIOPipe, ComponentLoadPipe,
        ComponentLoadWhileIOPipe, ComponentLoadable, ComponentRenderPipe, ComponentRenderable,
        ComponentUpdatable, ComponentUpdatePipe,
    },
    ui::{
        types::{UIFeedbackInterface, UIPipe, UIState},
        ui::UI,
    },
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

    pub fn render_stats(&mut self, ui: &mut egui::Ui, pipe: &mut UIPipe, _ui_state: &mut UIState) {
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

/**
 * This component collects various client statistics and displays them optionally
 */
pub struct ClientStats {
    data: ClientStatsData,
    ui: UI,
}

impl ClientStats {
    pub fn new(sys: &system::System) -> Self {
        let mut ui = UI::new(2.5);
        ui.set_main_panel_color(&Color32::TRANSPARENT);
        Self {
            data: ClientStatsData::new(sys),
            ui: ui,
        }
    }
}

impl ComponentLoadable for ClientStats {
    fn load_io(&mut self, _io_pipe: &mut ComponentLoadIOPipe) {}

    fn init_while_io(&mut self, _pipe: &mut ComponentLoadWhileIOPipe) {}

    fn init(&mut self, _pipe: &mut ComponentLoadPipe) -> Result<(), ArrayString<4096>> {
        Ok(())
    }
}

impl ComponentUpdatable for ClientStats {
    fn update(&mut self, _pipe: &mut ComponentUpdatePipe) {
        // TODO: some CPU frametime stats or smth similar
    }
}

impl ComponentRenderable for ClientStats {
    fn render(&mut self, pipe: &mut ComponentRenderPipe) {
        self.ui.render(
            |ui, pipe, ui_state| self.data.render_stats(ui, pipe, ui_state),
            &mut UIPipe {
                ui_feedback: &mut ClientStatsUIFeedbackDummy {},
                graphics: pipe.graphics,
                sys: pipe.sys,
                runtime_thread_pool: pipe.runtime_thread_pool,
                config: pipe.config,
            },
        )
    }
}

impl ComponentGameMsg for ClientStats {
    // TODO: network stats
    fn on_msg(
        &mut self,
        _timestamp: &std::time::Duration,
        _msg: &crate::network::messages::ServerToClientMessage,
        _pipe: &mut crate::client::component::GameMsgPipeline,
    ) {
    }

    fn on_connect(&mut self, _timestamp: &std::time::Duration) {}

    fn on_disconnect(&mut self, _timestamp: &std::time::Duration) {}
}

impl ComponentComponent for ClientStats {
    fn does_update(&self) -> bool {
        true
    }
    fn does_render(&self) -> bool {
        true
    }
    fn handles_msgs(&self) -> bool {
        true
    }
}
