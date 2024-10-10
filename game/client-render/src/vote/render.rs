use std::time::Duration;

use client_containers::skins::SkinContainer;
use client_render_base::render::tee::RenderTee;
use client_ui::vote::{
    page::VoteUi,
    user_data::{UserData, VoteRenderData},
};
use egui::{Color32, Rect};
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

pub struct VoteRenderPipe<'a> {
    pub cur_time: &'a Duration,
    pub skin_container: &'a mut SkinContainer,
    pub tee_render: &'a mut RenderTee,

    pub vote_data: VoteRenderData<'a>,
}

pub struct VoteRender {
    pub ui: UiContainer,
    vote_ui: VoteUi,

    backend_handle: GraphicsBackendHandle,
    canvas_handle: GraphicsCanvasHandle,
    stream_handle: GraphicsStreamHandle,
    texture_handle: GraphicsTextureHandle,
}

impl VoteRender {
    pub fn new(graphics: &Graphics, creator: &UiCreator) -> Self {
        let mut ui = UiContainer::new(creator);
        ui.set_main_panel_color(&Color32::TRANSPARENT);
        Self {
            ui,
            vote_ui: VoteUi::new(),

            backend_handle: graphics.backend_handle.clone(),
            canvas_handle: graphics.canvas_handle.clone(),
            stream_handle: graphics.stream_handle.clone(),
            texture_handle: graphics.texture_handle.clone(),
        }
    }

    /// Returns an optional rect where a voted player could be rendered.
    pub fn render(&mut self, pipe: &mut VoteRenderPipe) -> Option<Rect> {
        let mut player_rect = None;
        let mut user_data = UserData {
            canvas_handle: &self.canvas_handle,
            stream_handle: &self.stream_handle,
            skin_container: pipe.skin_container,
            render_tee: pipe.tee_render,

            vote_data: pipe.vote_data,
            player_vote_rect: &mut player_rect,
        };
        let mut dummy_pipe = UiRenderPipe::new(*pipe.cur_time, &mut user_data);
        generic_ui_renderer::render(
            &self.backend_handle,
            &self.texture_handle,
            &self.stream_handle,
            &self.canvas_handle,
            &mut self.ui,
            &mut self.vote_ui,
            &mut dummy_pipe,
            Default::default(),
            Default::default(),
        );
        player_rect
    }
}
