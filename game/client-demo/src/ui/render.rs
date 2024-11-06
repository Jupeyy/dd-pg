use std::time::Duration;

use client_render::generic_ui_renderer;
use client_ui::demo_player::{page::DemoPlayerUi, user_data::UserData};
use egui::Color32;
use graphics::{
    graphics::graphics::Graphics,
    handles::{
        backend::backend::GraphicsBackendHandle, canvas::canvas::GraphicsCanvasHandle,
        stream::stream::GraphicsStreamHandle, texture::texture::GraphicsTextureHandle,
    },
};

use ui_base::{
    types::UiRenderPipe,
    ui::{UiContainer, UiCreator},
};

pub struct DemoPlayerUiRenderPipe<'a> {
    pub cur_time: &'a Duration,

    pub player_info: UserData<'a>,
}

pub struct DemoPlayerUiRender {
    ui: UiContainer,
    demo_player_ui: DemoPlayerUi,

    backend_handle: GraphicsBackendHandle,
    canvas_handle: GraphicsCanvasHandle,
    stream_handle: GraphicsStreamHandle,
    texture_handle: GraphicsTextureHandle,
}

impl DemoPlayerUiRender {
    pub fn new(graphics: &Graphics, creator: &UiCreator) -> Self {
        let mut ui = UiContainer::new(creator);
        ui.set_main_panel_color(&Color32::TRANSPARENT);
        Self {
            ui,
            demo_player_ui: DemoPlayerUi::new(),
            backend_handle: graphics.backend_handle.clone(),
            canvas_handle: graphics.canvas_handle.clone(),
            stream_handle: graphics.stream_handle.clone(),
            texture_handle: graphics.texture_handle.clone(),
        }
    }

    pub fn render(&mut self, pipe: &mut DemoPlayerUiRenderPipe, input: egui::RawInput) {
        generic_ui_renderer::render(
            &self.backend_handle,
            &self.texture_handle,
            &self.stream_handle,
            &self.canvas_handle,
            &mut self.ui,
            &mut self.demo_player_ui,
            &mut UiRenderPipe::new(*pipe.cur_time, &mut pipe.player_info),
            Default::default(),
            input,
        );
    }
}
