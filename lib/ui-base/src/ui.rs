use std::time::Duration;

use egui::{epaint::ahash::HashMap, Color32, TextureId};
use graphics_types::textures_handle::TextureIndex;

use crate::types::{UINativePipe, UINativeState, UIRawInputGenerator};

use super::types::{UIPipe, UIState};

pub fn gui_main_panel(main_panel_color: &Color32) -> egui::CentralPanel {
    let standard_frame = egui::containers::Frame {
        inner_margin: egui::style::Margin {
            left: 0.,
            right: 0.,
            top: 0.,
            bottom: 0.,
        },
        outer_margin: egui::style::Margin {
            left: 0.,
            right: 0.,
            top: 0.,
            bottom: 0.,
        },
        rounding: egui::Rounding {
            nw: 0.0,
            ne: 0.0,
            sw: 0.0,
            se: 0.0,
        },
        shadow: egui::epaint::Shadow::NONE,
        fill: *main_panel_color,
        stroke: egui::Stroke::NONE,
    };
    egui::CentralPanel::default().frame(standard_frame)
}

pub trait UIInterface<T> {
    /// `zoom_level` is optional. and means that it overrides the default value
    /// which is calculated by the window
    fn new(state: T, zoom_level: Option<f32>) -> Self;

    fn split_mut(
        &mut self,
        as_stencil: bool,
    ) -> (
        &mut UIState,
        &mut UINativeState<T>,
        &mut egui::Context,
        &mut Color32,
    );

    fn set_main_panel_color(&mut self, main_panel_color: &Color32) {
        *self.split_mut(false).3 = *main_panel_color;
    }

    /// returns the canvas rect, full output and current zoom level
    fn render<U>(
        &mut self,
        window_width: u32,
        window_height: u32,
        window_pixels_per_point: f32,
        render_func: impl FnOnce(&mut egui::Ui, &mut UIPipe<U>, &mut UIState),
        pipe: &mut UIPipe<U>,
        native_pipe: &mut UINativePipe<T>,
        as_stencil: bool,
    ) -> (egui::Rect, egui::FullOutput, f32) {
        let (ui_state, ui_native_state, egui_ctx, main_panel_color) = self.split_mut(as_stencil);
        let mut zoom_level = ui_state.zoom_level.unwrap_or(window_pixels_per_point);
        // difference between the zoom level (either by user or window) and the zoom level by the window
        // which is important to recalculate coordinates, like mouse position
        let user_zoom_level_difference = zoom_level / window_pixels_per_point;
        // Gather input (mouse, touches, keyboard, screen size, etc):
        let mut raw_input = native_pipe.raw_inp_generator.get_raw_input(ui_native_state);

        // first go through all events
        let mut hint_has_text_input = false;
        // scale the input events down
        raw_input.events.iter_mut().for_each(|ev| match ev {
            egui::Event::PointerMoved(ev) => {
                *ev = egui::pos2(
                    ev.x / user_zoom_level_difference,
                    ev.y / user_zoom_level_difference,
                )
            }
            egui::Event::PointerButton {
                pos,
                button: _,
                pressed: _,
                modifiers: _,
            } => {
                *pos = egui::pos2(
                    pos.x / user_zoom_level_difference,
                    pos.y / user_zoom_level_difference,
                )
            }
            egui::Event::Text(_) => {
                hint_has_text_input = true;
            }
            egui::Event::Zoom(extra_zoom_level) => {
                let incr_val = if *extra_zoom_level > 1.0 {
                    if zoom_level < 1.5 {
                        0.25
                    } else {
                        0.5
                    }
                } else if *extra_zoom_level < 1.0 {
                    if zoom_level > 1.5 {
                        -0.5
                    } else {
                        -0.25
                    }
                } else {
                    0.0
                };
                zoom_level = (zoom_level + incr_val)
                    .clamp(window_pixels_per_point - 0.5, window_pixels_per_point + 1.0);
            }
            _ => {}
        });
        ui_state.hint_had_input = hint_has_text_input;

        let screen_rect = egui::Rect {
            min: egui::Pos2 { x: 0.0, y: 0.0 },
            max: egui::Pos2 {
                x: window_width as f32 / zoom_level,
                y: window_height as f32 / zoom_level,
            },
        };
        raw_input.screen_rect = if screen_rect.width() > 0.0 && screen_rect.height() > 0.0 {
            Some(screen_rect)
        } else {
            None
        };
        raw_input.pixels_per_point = Some(zoom_level);
        let cur_time_secs =
            pipe.cur_time.as_nanos() as f64 / (Duration::from_secs(1).as_nanos() as f64);
        raw_input.time = Some(cur_time_secs);

        (
            screen_rect,
            egui_ctx.run(raw_input, |egui_ctx| {
                gui_main_panel(&main_panel_color)
                    .show(egui_ctx, |ui| render_func(ui, pipe, ui_state));
            }),
            zoom_level,
        )
    }
}

/// this is useful if you only care to render using the UI, no input
#[derive(Default)]
pub struct UIDummyState {}

pub struct UIDummyRawInputGenerator {}

impl UIRawInputGenerator<UIDummyState> for UIDummyRawInputGenerator {
    fn get_raw_input(&self, _state: &mut UINativeState<UIDummyState>) -> egui::RawInput {
        egui::RawInput::default()
    }
    fn process_output(
        &self,
        _state: &mut UINativeState<UIDummyState>,
        _ctx: &egui::Context,
        _output: egui::PlatformOutput,
    ) {
    }
}

#[derive(Debug, Default)]
pub struct UIContext {
    pub egui_ctx: egui::Context,
    pub textures: HashMap<TextureId, TextureIndex>,
}

/**
 * UI is not a client component, it should be cleanly separated from any game logic (but can read it)
 */
pub struct UI<T> {
    pub context: UIContext,
    pub stencil_context: UIContext,

    pub ui_state: UIState,
    pub ui_native_state: UINativeState<T>,

    pub main_panel_color: Color32,
}

impl<T> UIInterface<T> for UI<T> {
    fn new(state: T, zoom_level: Option<f32>) -> Self {
        let res = Self {
            context: Default::default(),
            stencil_context: Default::default(),

            ui_state: UIState::new(zoom_level),
            ui_native_state: UINativeState::new(state),

            main_panel_color: Color32::TRANSPARENT,
        };
        let vis = egui::style::Visuals::dark();
        res.context.egui_ctx.set_visuals(vis.clone());
        res.stencil_context.egui_ctx.set_visuals(vis);
        res
    }

    fn split_mut(
        &mut self,
        as_stencil: bool,
    ) -> (
        &mut UIState,
        &mut UINativeState<T>,
        &mut egui::Context,
        &mut Color32,
    ) {
        (
            &mut self.ui_state,
            &mut self.ui_native_state,
            if as_stencil {
                &mut self.stencil_context.egui_ctx
            } else {
                &mut self.context.egui_ctx
            },
            &mut self.main_panel_color,
        )
    }
}
