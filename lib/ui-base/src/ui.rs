use std::time::Duration;

use base::system::SystemTimeInterface;
use egui::{
    epaint::ahash::{HashMap, HashMapExt},
    Color32, TextureId,
};

use graphics_types::rendering::TextureIndex;

use crate::types::UIRawInputGenerator;

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
    fn new(state: T, zoom_level: f32) -> Self;

    fn split_mut(&mut self) -> (&mut UIState<T>, &mut egui::Context, &mut Color32);

    fn set_main_panel_color(&mut self, main_panel_color: &Color32) {
        *self.split_mut().2 = *main_panel_color;
    }

    fn render(
        &mut self,
        canvas_width: u32,
        canvas_height: u32,
        render_func: impl FnOnce(&mut egui::Ui, &mut UIPipe<T>, &mut UIState<T>),
        pipe: &mut UIPipe<T>,
    ) -> (egui::Rect, egui::FullOutput) {
        let (ui_state, egui_ctx, main_panel_color) = self.split_mut();
        // Gather input (mouse, touches, keyboard, screen size, etc):
        let mut raw_input = pipe.raw_inp_generator.get_raw_input(ui_state);
        let screen_rect = egui::Rect {
            min: egui::Pos2 { x: 0.0, y: 0.0 },
            max: egui::Pos2 {
                x: canvas_width as f32 / ui_state.zoom_level,
                y: canvas_height as f32 / ui_state.zoom_level,
            },
        };
        raw_input.screen_rect = Some(screen_rect);
        raw_input.pixels_per_point = Some(ui_state.zoom_level);
        let cur_time_secs = pipe.sys.time_get_nanoseconds().as_nanos() as f64
            / (Duration::from_secs(1).as_nanos() as f64);
        raw_input.time = Some(cur_time_secs);

        // scale the input events down
        raw_input.events.iter_mut().for_each(|ev| match ev {
            egui::Event::PointerMoved(ev) => {
                *ev = egui::pos2(ev.x / ui_state.zoom_level, ev.y / ui_state.zoom_level)
            }
            egui::Event::PointerButton {
                pos,
                button: _,
                pressed: _,
                modifiers: _,
            } => *pos = egui::pos2(pos.x / ui_state.zoom_level, pos.y / ui_state.zoom_level),
            _ => {}
        });

        (
            screen_rect,
            egui_ctx.run(raw_input, |egui_ctx| {
                gui_main_panel(&main_panel_color)
                    .show(egui_ctx, |ui| render_func(ui, pipe, ui_state));
            }),
        )
    }
}

/// this is useful if you only care to render using the UI, no input
#[derive(Default)]
pub struct UIDummyState {}

pub struct UIDummyRawInputGenerator {}

impl UIRawInputGenerator<UIDummyState> for UIDummyRawInputGenerator {
    fn get_raw_input(&self, _state: &mut UIState<UIDummyState>) -> egui::RawInput {
        egui::RawInput::default()
    }
}

/**
 * UI is not a client component, it should be cleanly separated from any game logic (but can read it)
 */
pub struct UI<T> {
    pub egui_ctx: egui::Context,
    pub textures: HashMap<TextureId, TextureIndex>,

    pub draw_ranges: Vec<(usize, usize, usize, TextureId, egui::Rect)>,
    pub mesh_index_offsets: Vec<usize>,

    pub ui_state: UIState<T>,

    pub main_panel_color: Color32,
}

impl<T> UIInterface<T> for UI<T> {
    fn new(state: T, zoom_level: f32) -> Self {
        let res = Self {
            egui_ctx: egui::Context::default(),

            textures: HashMap::new(),

            ui_state: UIState::new(state, zoom_level),

            draw_ranges: Vec::new(),
            mesh_index_offsets: Vec::new(),

            main_panel_color: Color32::TRANSPARENT,
        };
        let vis = egui::style::Visuals::dark();
        res.egui_ctx.set_visuals(vis);
        res
    }

    fn split_mut(&mut self) -> (&mut UIState<T>, &mut egui::Context, &mut Color32) {
        (
            &mut self.ui_state,
            &mut self.egui_ctx,
            &mut self.main_panel_color,
        )
    }
}
