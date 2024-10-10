use std::{
    sync::{atomic::AtomicU64, Arc},
    time::Duration,
};

use egui::Color32;
use egui_extras::StripBuilder;

use graphics::graphics::graphics::Graphics;

use math::math::vector::luffixed;
use prediction_timer::prediction_timing::PredictionTimer;
use ui_base::{
    style::default_style,
    types::UiRenderPipe,
    ui::{UiContainer, UiCreator},
    ui_render::render_ui,
};

use base::system::{self, SystemTimeInterface};

use crate::game::data::NetworkByteStats;

pub struct DebugHudData {
    texture_memory_usage: Arc<AtomicU64>,
    buffer_memory_usage: Arc<AtomicU64>,
    stream_memory_usage: Arc<AtomicU64>,
    staging_memory_usage: Arc<AtomicU64>,
}

impl DebugHudData {
    pub fn new(
        texture_memory_usage: Arc<AtomicU64>,
        buffer_memory_usage: Arc<AtomicU64>,
        stream_memory_usage: Arc<AtomicU64>,
        staging_memory_usage: Arc<AtomicU64>,
    ) -> Self {
        Self {
            texture_memory_usage,
            buffer_memory_usage,
            stream_memory_usage,
            staging_memory_usage,
        }
    }

    pub fn render_stats(
        &mut self,
        ui: &mut egui::Ui,
        pipe: &mut UiRenderPipe<DebugHudRenderPipe<'_>>,
    ) {
        ui.set_style(default_style());

        StripBuilder::new(ui)
            .size(egui_extras::Size::remainder())
            .size(egui_extras::Size::exact(100.0))
            .horizontal(|mut strip| {
                strip.cell(|_| {});
                strip.cell(|ui| {
                    ui.add_space(20.0);

                    ui.painter()
                        .rect_filled(ui.available_rect_before_wrap(), 5.0, Color32::BLACK);

                    let timing = pipe.user_data.prediction_timer.snapshot();
                    ui.label("Network");
                    ui.label("Ping (ms):");
                    ui.colored_label(
                        Color32::from_rgb(255, 0, 255),
                        format!(
                            "{:.2}",
                            pipe.user_data.prediction_timer.ping_average().as_micros() as f64
                                / 1000.0
                        ),
                    );
                    ui.label("Min-Ping (ms):");
                    ui.colored_label(
                        Color32::from_rgb(255, 0, 255),
                        format!(
                            "{:.2}",
                            pipe.user_data.prediction_timer.ping_min().as_micros() as f64 / 1000.0
                        ),
                    );
                    ui.label("Max-Ping (ms):");
                    ui.colored_label(
                        Color32::from_rgb(255, 0, 255),
                        format!(
                            "{:.2}",
                            pipe.user_data.prediction_timer.ping_max().as_micros() as f64 / 1000.0
                        ),
                    );
                    ui.label("Ping jitter (ms):");
                    ui.colored_label(
                        Color32::from_rgb(255, 0, 255),
                        format!(
                            "{:.2}",
                            pipe.user_data.prediction_timer.jitter_range.as_micros() as f64
                                / 1000.0
                        ),
                    );
                    ui.label("Max frame time (ms):");
                    ui.colored_label(
                        Color32::from_rgb(255, 0, 255),
                        format!(
                            "{:.2}",
                            pipe.user_data.prediction_timer.max_frametime().as_micros() as f64
                                / 1000.0
                        ),
                    );
                    ui.label("Prediction adjustment smooth (ms):");
                    ui.colored_label(
                        Color32::from_rgb(255, 0, 255),
                        format!("{:.4}", timing.smooth_adjustment_time * 1000.0),
                    );
                    ui.label("Ingame time (ms):");
                    ui.colored_label(
                        Color32::from_rgb(255, 0, 255),
                        format!(
                            "{:.2}",
                            pipe.user_data.ingame_timer.as_micros() as f64 / 1000.0
                        ),
                    );
                    ui.label("Packet loss (sending) %:");
                    ui.colored_label(
                        Color32::from_rgb(255, 0, 255),
                        format!(
                            "{:.2}",
                            pipe.user_data.prediction_timer.packet_loss() * 100.0
                        ),
                    );

                    ui.label("Sent Kibit/s:");
                    ui.colored_label(
                        Color32::from_rgb(255, 0, 255),
                        format!(
                            "{:.2}",
                            (pipe.user_data.byte_stats.bytes_per_sec_sent * luffixed::from_num(8))
                                / luffixed::from_num(1024)
                        ),
                    );
                    ui.label("Recv Kibit/s:");
                    ui.colored_label(
                        Color32::from_rgb(255, 0, 255),
                        format!(
                            "{:.2}",
                            (pipe.user_data.byte_stats.bytes_per_sec_recv * luffixed::from_num(8))
                                / luffixed::from_num(1024)
                        ),
                    );

                    ui.label("Graphics");
                    ui.label("Texture usage MiB:");
                    ui.colored_label(
                        Color32::from_rgb(255, 0, 255),
                        format!(
                            "{:.6}",
                            (self
                                .texture_memory_usage
                                .load(std::sync::atomic::Ordering::Relaxed)
                                as f64
                                / 1024.0
                                / 1024.0)
                        ),
                    );

                    ui.label("Buffer usage MiB:");
                    ui.colored_label(
                        Color32::from_rgb(255, 0, 255),
                        format!(
                            "{:.6}",
                            (self
                                .buffer_memory_usage
                                .load(std::sync::atomic::Ordering::Relaxed)
                                as f64
                                / 1024.0
                                / 1024.0)
                        ),
                    );

                    ui.label("Stream usage MiB:");
                    ui.colored_label(
                        Color32::from_rgb(255, 0, 255),
                        format!(
                            "{:.6}",
                            (self
                                .stream_memory_usage
                                .load(std::sync::atomic::Ordering::Relaxed)
                                as f64
                                / 1024.0
                                / 1024.0)
                        ),
                    );

                    ui.label("Staging usage MiB:");
                    ui.colored_label(
                        Color32::from_rgb(255, 0, 255),
                        format!(
                            "{:.6}",
                            (self
                                .staging_memory_usage
                                .load(std::sync::atomic::Ordering::Relaxed)
                                as f64
                                / 1024.0
                                / 1024.0)
                        ),
                    );
                })
            });
    }
}

pub struct DebugHudRenderPipe<'a> {
    pub graphics: &'a Graphics,
    pub prediction_timer: &'a PredictionTimer,
    pub byte_stats: &'a NetworkByteStats,
    pub ingame_timer: &'a Duration,
}

/// This component collects various client statistics and displays them optionally
pub struct DebugHud {
    data: DebugHudData,
    pub ui: UiContainer,
    sys: Arc<dyn SystemTimeInterface>,
}

impl DebugHud {
    pub fn new(
        sys: &system::System,
        texture_memory_usage: Arc<AtomicU64>,
        buffer_memory_usage: Arc<AtomicU64>,
        stream_memory_usage: Arc<AtomicU64>,
        staging_memory_usage: Arc<AtomicU64>,
        creator: &UiCreator,
    ) -> Self {
        let mut ui = UiContainer::new(creator);
        ui.set_main_panel_color(&Color32::TRANSPARENT);
        ui.ui_state.is_ui_open = false;
        Self {
            data: DebugHudData::new(
                texture_memory_usage,
                buffer_memory_usage,
                stream_memory_usage,
                staging_memory_usage,
            ),
            ui,
            sys: sys.time.clone(),
        }
    }

    pub fn render(&mut self, pipe: &mut DebugHudRenderPipe) {
        let window_width = pipe.graphics.canvas_handle.window_width();
        let window_height = pipe.graphics.canvas_handle.window_height();
        let window_pixels_per_point = pipe.graphics.canvas_handle.window_pixels_per_point();
        let (screen_rect, full_output, zoom_level) = self.ui.render(
            window_width,
            window_height,
            window_pixels_per_point,
            |ui, pipe, _| self.data.render_stats(ui, pipe),
            &mut UiRenderPipe::new(self.sys.time_get_nanoseconds(), pipe),
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
