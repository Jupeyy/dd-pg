use std::time::Duration;

use egui::Color32;
use egui_extras::StripBuilder;
use fixed::{types::extra::U16, FixedI64};

use graphics::graphics::graphics::Graphics;

use ui_base::{
    style::default_style,
    types::{UiRenderPipe, UiState},
    ui::{UiContainer, UiCreator},
    ui_render::render_ui,
};

use math::math::blend;

use base::system::{self, SystemTimeInterface};

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
        pipe: &mut UiRenderPipe<()>,
        _ui_state: &mut UiState,
    ) {
        ui.set_style(default_style());
        let cur_time = pipe.cur_time;
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
    pub graphics: &'a Graphics,
    pub sys: &'a system::System,
}

/// This component collects various client statistics and displays them optionally
pub struct ClientStats {
    data: ClientStatsData,
    ui: UiContainer,
}

impl ClientStats {
    pub fn new(sys: &system::System, creator: &UiCreator) -> Self {
        let mut ui = UiContainer::new(creator);
        ui.set_main_panel_color(&Color32::TRANSPARENT);
        Self {
            data: ClientStatsData::new(sys),
            ui,
        }
    }

    pub fn render(&mut self, pipe: &mut ClientStatsRenderPipe) {
        let window_width = pipe.graphics.canvas_handle.window_width();
        let window_height = pipe.graphics.canvas_handle.window_height();
        let window_pixels_per_point = pipe.graphics.canvas_handle.window_pixels_per_point();
        let (screen_rect, full_output, zoom_level) = self.ui.render(
            window_width,
            window_height,
            window_pixels_per_point,
            |ui, pipe, ui_state| self.data.render_stats(ui, pipe, ui_state),
            &mut UiRenderPipe::new(pipe.sys.time_get_nanoseconds(), &mut ()),
            Default::default(),
            false,
        );
        render_ui(
            &mut self.ui,
            full_output,
            &screen_rect,
            zoom_level,
            &pipe.graphics.backend_handle,
            &pipe.graphics.texture_handle,
            &pipe.graphics.stream_handle,
            false,
        );
    }
}
