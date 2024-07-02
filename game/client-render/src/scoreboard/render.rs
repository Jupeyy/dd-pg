use std::time::Duration;

use client_containers_new::skins::SkinContainer;
use client_render_base::render::tee::RenderTee;
use client_ui::scoreboard::{page::ScoreboardUi, user_data::UserData};
use egui::Color32;
use graphics::{
    graphics::graphics::Graphics,
    handles::{
        backend::backend::GraphicsBackendHandle, canvas::canvas::GraphicsCanvasHandle,
        stream::stream::GraphicsStreamHandle, texture::texture::GraphicsTextureHandle,
    },
};

use game_interface::types::{
    game::GameEntityId,
    render::{character::CharacterInfo, scoreboard::ScoreboardGameType},
};
use hashlink::LinkedHashMap;
use ui_base::{types::UiRenderPipe, ui::UiContainer};

use crate::generic_ui_renderer;

pub struct ScoreboardRenderPipe<'a> {
    pub cur_time: &'a Duration,
    pub entries: &'a ScoreboardGameType,
    pub character_infos: &'a LinkedHashMap<GameEntityId, CharacterInfo>,
    pub skin_container: &'a mut SkinContainer,
    pub tee_render: &'a mut RenderTee,
}

pub struct ScoreboardRender {
    ui: UiContainer,
    scoreboard_ui: ScoreboardUi,

    backend_handle: GraphicsBackendHandle,
    canvas_handle: GraphicsCanvasHandle,
    stream_handle: GraphicsStreamHandle,
    texture_handle: GraphicsTextureHandle,
}

impl ScoreboardRender {
    pub fn new(graphics: &Graphics) -> Self {
        let mut ui = UiContainer::new(None);
        ui.set_main_panel_color(&Color32::TRANSPARENT);
        Self {
            ui,
            scoreboard_ui: ScoreboardUi::new(),
            backend_handle: graphics.backend_handle.clone(),
            canvas_handle: graphics.canvas_handle.clone(),
            stream_handle: graphics.stream_handle.clone(),
            texture_handle: graphics.texture_handle.clone(),
        }
    }

    pub fn render(&mut self, pipe: &mut ScoreboardRenderPipe) {
        generic_ui_renderer::render(
            &self.backend_handle,
            &self.texture_handle,
            &self.stream_handle,
            &self.canvas_handle,
            &mut self.ui,
            &mut self.scoreboard_ui,
            &mut UiRenderPipe::new(
                *pipe.cur_time,
                &mut UserData {
                    game_data: pipe.entries,
                    character_infos: pipe.character_infos,
                    canvas_handle: &self.canvas_handle,
                    skin_container: pipe.skin_container,
                    render_tee: pipe.tee_render,
                },
            ),
            Default::default(),
            Default::default(),
        );
    }
}
