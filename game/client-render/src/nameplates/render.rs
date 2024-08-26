use std::time::Duration;

use egui::{pos2, Align2, Color32, FontId};
use graphics::{
    graphics::graphics::Graphics,
    handles::{
        backend::backend::GraphicsBackendHandle, canvas::canvas::GraphicsCanvasHandle,
        stream::stream::GraphicsStreamHandle, texture::texture::GraphicsTextureHandle,
    },
};

use graphics_types::rendering::State;
use math::math::vector::vec2;
use ui_base::{
    types::UiRenderPipe,
    ui::{UiContainer, UiCreator},
    ui_render::render_ui,
};

pub struct NameplateRenderPipe<'a> {
    pub cur_time: &'a Duration,
    pub name: &'a str,
    pub state: &'a State,
    pub pos: &'a vec2,
}

pub struct NameplateRender {
    ui: UiContainer,

    canvas_handle: GraphicsCanvasHandle,
    backend_handle: GraphicsBackendHandle,
    texture_handle: GraphicsTextureHandle,
    stream_handle: GraphicsStreamHandle,
}

impl NameplateRender {
    pub fn new(graphics: &Graphics, creator: &UiCreator) -> Self {
        let mut ui = UiContainer::new(None, creator);
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

        let (screen_rect, full_output, zoom_level) = self.ui.render_cached(
            window_width,
            window_height,
            window_pixels_per_point,
            |ui, _inner_pipe, _ui_state| {
                let size = ui.ctx().screen_rect().size();
                let (x0, y0, x1, y1) = pipe.state.get_canvas_mapping();

                let name_scale = size.x / self.canvas_handle.canvas_width();

                let w = x1 - x0;
                let h = y1 - y0;

                let width_scale = size.x / w;
                let height_scale = size.y / h;
                ui.painter().text(
                    pos2(
                        (pipe.pos.x - x0) * width_scale,
                        (pipe.pos.y - y0 - 70.0 / 64.0) * height_scale,
                    ),
                    Align2::CENTER_BOTTOM,
                    pipe.name,
                    FontId::proportional((1.0 * name_scale) * height_scale),
                    Color32::WHITE,
                );
            },
            &mut dummy_pipe,
            Default::default(),
            false,
            true,
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
