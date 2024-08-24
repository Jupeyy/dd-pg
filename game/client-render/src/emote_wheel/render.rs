use std::time::Duration;

use client_containers::{emoticons::EmoticonsContainer, skins::SkinContainer};
use client_render_base::render::tee::RenderTee;
use client_ui::emote_wheel::{
    page::EmoteWheelUi,
    user_data::{EmoteWheelEvent, UserData},
};
use egui::Color32;
use game_interface::types::{character_info::NetworkSkinInfo, resource_key::ResourceKey};
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

use crate::generic_ui_renderer;

pub struct EmoteWheelRenderPipe<'a> {
    pub cur_time: &'a Duration,
    pub input: &'a mut Option<egui::RawInput>,
    pub skin_container: &'a mut SkinContainer,
    pub emoticons_container: &'a mut EmoticonsContainer,
    pub tee_render: &'a mut RenderTee,

    pub emoticons: &'a ResourceKey,
    pub skin: &'a ResourceKey,
    pub skin_info: &'a Option<NetworkSkinInfo>,
}

pub struct EmoteWheelRender {
    pub ui: UiContainer,
    emote_wheel_ui: EmoteWheelUi,

    backend_handle: GraphicsBackendHandle,
    canvas_handle: GraphicsCanvasHandle,
    stream_handle: GraphicsStreamHandle,
    texture_handle: GraphicsTextureHandle,
}

impl EmoteWheelRender {
    pub fn new(graphics: &Graphics, creator: &UiCreator) -> Self {
        let mut ui = UiContainer::new(None, creator);
        ui.set_main_panel_color(&Color32::TRANSPARENT);
        Self {
            ui,
            emote_wheel_ui: EmoteWheelUi::new(),

            backend_handle: graphics.backend_handle.clone(),
            canvas_handle: graphics.canvas_handle.clone(),
            stream_handle: graphics.stream_handle.clone(),
            texture_handle: graphics.texture_handle.clone(),
        }
    }

    pub fn render(&mut self, pipe: &mut EmoteWheelRenderPipe) -> Vec<EmoteWheelEvent> {
        let mut res: Vec<EmoteWheelEvent> = Default::default();
        let mut user_data = UserData {
            events: &mut res,
            canvas_handle: &self.canvas_handle,
            stream_handle: &self.stream_handle,
            skin_container: pipe.skin_container,
            emoticons_container: pipe.emoticons_container,
            render_tee: pipe.tee_render,

            emoticon: pipe.emoticons,
            skin: pipe.skin,
            skin_info: pipe.skin_info,
        };
        let mut dummy_pipe = UiRenderPipe::new(*pipe.cur_time, &mut user_data);
        generic_ui_renderer::render(
            &self.backend_handle,
            &self.texture_handle,
            &self.stream_handle,
            &self.canvas_handle,
            &mut self.ui,
            &mut self.emote_wheel_ui,
            &mut dummy_pipe,
            Default::default(),
            pipe.input.take().unwrap_or_default(),
        );
        res
    }
}
