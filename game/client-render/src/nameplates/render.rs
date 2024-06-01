use std::time::Duration;

use egui::Color32;
use graphics::{
    graphics::graphics::Graphics,
    handles::{
        backend::backend::GraphicsBackendHandle, canvas::canvas::GraphicsCanvasHandle,
        stream::stream::GraphicsStreamHandle, texture::texture::GraphicsTextureHandle,
    },
};

use ui_base::{types::UiRenderPipe, ui::UiContainer, ui_render::render_ui};

pub struct NameplateRenderPipe<'a> {
    pub cur_time: &'a Duration,
    pub name: &'a str,
}

pub struct NameplateRender {
    ui: UiContainer,

    canvas_handle: GraphicsCanvasHandle,
    backend_handle: GraphicsBackendHandle,
    texture_handle: GraphicsTextureHandle,
    stream_handle: GraphicsStreamHandle,
}

impl NameplateRender {
    pub fn new(graphics: &Graphics) -> Self {
        let mut ui = UiContainer::new(None);
        ui.set_main_panel_color(&Color32::TRANSPARENT);
        Self {
            ui,
            backend_handle: graphics.backend_handle.clone(),
            canvas_handle: graphics.canvas_handle.clone(),
            stream_handle: graphics.stream_handle.clone(),
            texture_handle: graphics.texture_handle.clone(),
        }
    }

    pub fn render(&mut self, pipe: &mut NameplateRenderPipe) {
        let window_width = self.canvas_handle.window_width();
        let window_height = self.canvas_handle.window_height();
        let window_pixels_per_point = self.canvas_handle.window_pixels_per_point();

        let mut user_data = ();
        let mut dummy_pipe = UiRenderPipe::new(*pipe.cur_time, &mut user_data);

        let (screen_rect, full_output, zoom_level) = self.ui.render(
            window_width,
            window_height,
            window_pixels_per_point,
            |ui, _inner_pipe, _ui_state| {
                ui.label(pipe.name);
            },
            &mut dummy_pipe,
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
