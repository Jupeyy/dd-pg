use std::{collections::VecDeque, time::Duration};

use client_types::actionfeed::ActionFeed;
use client_ui::actionfeed::user_data::UserData;
use egui::Color32;
use graphics::{
    graphics::graphics::Graphics,
    handles::{
        backend::backend::GraphicsBackendHandle, canvas::canvas::GraphicsCanvasHandle,
        stream::stream::GraphicsStreamHandle, texture::texture::GraphicsTextureHandle,
    },
};

use ui_base::{types::UIPipe, ui::UI, ui_render::render_ui};

pub struct ActionfeedRenderPipe<'a> {
    pub cur_time: &'a Duration,
}

pub struct ActionfeedRender {
    ui: UI,

    pub msgs: VecDeque<ActionFeed>,

    backend_handle: GraphicsBackendHandle,
    canvas_handle: GraphicsCanvasHandle,
    stream_handle: GraphicsStreamHandle,
    texture_handle: GraphicsTextureHandle,
}

impl ActionfeedRender {
    pub fn new(graphics: &Graphics) -> Self {
        let mut ui = UI::new(None);
        ui.set_main_panel_color(&Color32::TRANSPARENT);
        Self {
            ui,

            msgs: VecDeque::new(),

            backend_handle: graphics.backend_handle.clone(),
            canvas_handle: graphics.canvas_handle.clone(),
            stream_handle: graphics.stream_handle.clone(),
            texture_handle: graphics.texture_handle.clone(),
        }
    }

    pub fn render(&mut self, pipe: &mut ActionfeedRenderPipe) {
        let window_width = self.canvas_handle.window_width();
        let window_height = self.canvas_handle.window_height();
        let window_pixels_per_point = self.canvas_handle.window_pixels_per_point();
        let (screen_rect, full_output, zoom_level) = self.ui.render(
            window_width,
            window_height,
            window_pixels_per_point,
            |ui, inner_pipe, ui_state| {
                client_ui::actionfeed::main_frame::render(
                    ui,
                    &mut UIPipe {
                        cur_time: inner_pipe.cur_time,
                        user_data: &mut UserData {
                            entries: &self.msgs,
                            stream_handle: &self.stream_handle,
                            canvas_handle: &self.canvas_handle,
                        },
                    },
                    ui_state,
                    false,
                )
            },
            &mut UIPipe::new(*pipe.cur_time, &mut ()),
            Default::default(),
            false,
        );
        render_ui(
            &mut self.ui,
            full_output,
            &screen_rect,
            zoom_level,
            &self.backend_handle,
            &self.texture_handle,
            &self.stream_handle,
            false,
        );
    }
}
