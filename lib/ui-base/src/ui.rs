use std::{collections::HashMap, time::Duration};

use egui::{Color32, TextureId};
use graphics::handles::texture::texture::TextureContainer;
use hiarc::Hiarc;

use crate::style::default_style;

use super::types::{UIPipe, UIState};

pub fn gui_main_panel(main_panel_color: &Color32) -> egui::CentralPanel {
    let standard_frame = egui::containers::Frame {
        inner_margin: egui::Margin {
            left: 0.,
            right: 0.,
            top: 0.,
            bottom: 0.,
        },
        outer_margin: egui::Margin {
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

#[derive(Debug, Hiarc, Default)]
pub struct UIContext {
    pub egui_ctx: egui::Context,
    pub textures: HashMap<TextureId, TextureContainer>,
}

/// UI is not a client component, it should be cleanly separated from any game logic (but can read it)
pub struct UI {
    pub context: UIContext,
    pub stencil_context: UIContext,

    pub ui_state: UIState,

    pub main_panel_color: Color32,
}

impl UI {
    /// `zoom_level` is optional. and means that it overrides the default value
    /// which is calculated by the window
    pub fn new(zoom_level: Option<f32>) -> Self {
        let context = egui::Context::default();
        context.options_mut(|option| option.zoom_with_keyboard = false);
        context.set_style(default_style());
        let stencil_context = egui::Context::default();
        stencil_context.options_mut(|option| option.zoom_with_keyboard = false);
        stencil_context.set_style(default_style());
        let res = Self {
            context: UIContext {
                egui_ctx: context,
                ..Default::default()
            },
            stencil_context: UIContext {
                egui_ctx: stencil_context,
                ..Default::default()
            },

            ui_state: UIState::new(zoom_level),

            main_panel_color: Color32::TRANSPARENT,
        };
        let vis = egui::style::Visuals::dark();
        res.context.egui_ctx.set_visuals(vis.clone());
        res.stencil_context.egui_ctx.set_visuals(vis);
        res
    }

    pub fn split_mut(
        &mut self,
        as_stencil: bool,
    ) -> (&mut UIState, &mut egui::Context, &mut Color32) {
        (
            &mut self.ui_state,
            if as_stencil {
                &mut self.stencil_context.egui_ctx
            } else {
                &mut self.context.egui_ctx
            },
            &mut self.main_panel_color,
        )
    }

    pub fn set_main_panel_color(&mut self, main_panel_color: &Color32) {
        *self.split_mut(false).2 = *main_panel_color;
    }

    /// returns the canvas rect, full output and current zoom level
    pub fn render<U>(
        &mut self,
        window_width: u32,
        window_height: u32,
        window_pixels_per_point: f32,
        render_func: impl FnOnce(&mut egui::Ui, &mut UIPipe<U>, &mut UIState),
        pipe: &mut UIPipe<U>,
        mut input: egui::RawInput,
        as_stencil: bool,
    ) -> (egui::Rect, egui::FullOutput, f32) {
        let (ui_state, egui_ctx, main_panel_color) = self.split_mut(as_stencil);
        let mut zoom_level = ui_state.zoom_level.unwrap_or(window_pixels_per_point);

        let zoom_diff = zoom_level / window_pixels_per_point;

        // first go through all events
        let mut hint_has_text_input = false;
        // scale the input events down
        input.events.retain_mut(|ev| match ev {
            egui::Event::PointerMoved(ev) => {
                *ev = egui::pos2(ev.x, ev.y) / zoom_diff;
                true
            }
            egui::Event::PointerButton {
                pos,
                button: _,
                pressed: _,
                modifiers: _,
            } => {
                *pos = egui::pos2(pos.x, pos.y) / zoom_diff;
                true
            }
            egui::Event::Text(_) => {
                hint_has_text_input = true;
                true
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
                false
            }
            _ => true,
        });
        ui_state.hint_had_input = hint_has_text_input;

        let screen_rect = egui::Rect {
            min: egui::Pos2 { x: 0.0, y: 0.0 },
            max: egui::Pos2 {
                x: window_width as f32 / zoom_level,
                y: window_height as f32 / zoom_level,
            },
        };
        input.screen_rect = if screen_rect.width() > 0.0 && screen_rect.height() > 0.0 {
            Some(screen_rect)
        } else {
            None
        };
        let cur_time_secs =
            pipe.cur_time.as_nanos() as f64 / (Duration::from_secs(1).as_nanos() as f64);
        input.time = Some(cur_time_secs);

        input.viewport_id = egui_ctx.viewport_id();
        input.viewports.insert(
            egui_ctx.viewport_id(),
            egui::ViewportInfo {
                parent: Default::default(),
                title: Default::default(),
                events: Default::default(),
                native_pixels_per_point: Some(zoom_level),
                monitor_size: Default::default(),
                inner_rect: Default::default(),
                outer_rect: Default::default(),
                minimized: Default::default(),
                maximized: Default::default(),
                fullscreen: Default::default(),
                focused: Default::default(),
            },
        );
        if zoom_level == window_pixels_per_point {
            ui_state.zoom_level = None;
        } else {
            ui_state.zoom_level = Some(zoom_level);
        }
        (
            screen_rect,
            egui_ctx.run(input, |egui_ctx| {
                gui_main_panel(&main_panel_color)
                    .show(egui_ctx, |ui| render_func(ui, pipe, ui_state));
            }),
            zoom_level,
        )
    }
}
