use std::{collections::VecDeque, time::Duration};

use client_containers::skins::SkinContainer;
use client_render_base::render::tee::RenderTee;
use client_ui::spectator_selection::{
    page::SpectatorSelectionUi,
    user_data::{SpectatorSelectionEvent, UserData},
};
use egui::Color32;
use game_interface::types::{id_types::CharacterId, render::character::CharacterInfo};
use graphics::{
    graphics::graphics::Graphics,
    handles::{
        backend::backend::GraphicsBackendHandle, canvas::canvas::GraphicsCanvasHandle,
        stream::stream::GraphicsStreamHandle, texture::texture::GraphicsTextureHandle,
    },
};
use hashlink::LinkedHashMap;
use ui_base::{
    types::UiRenderPipe,
    ui::{UiContainer, UiCreator},
    ui_render::render_ui,
};
use ui_traits::traits::UiPageInterface;

pub struct SpectatorSelectionRenderPipe<'a> {
    pub cur_time: &'a Duration,
    pub input: &'a mut Option<egui::RawInput>,
    pub skin_container: &'a mut SkinContainer,
    pub skin_renderer: &'a RenderTee,
    pub character_infos: &'a LinkedHashMap<CharacterId, CharacterInfo>,
}

pub struct SpectatorSelectionRender {
    pub ui: UiContainer,
    spectator_selection_ui: SpectatorSelectionUi,

    backend_handle: GraphicsBackendHandle,
    canvas_handle: GraphicsCanvasHandle,
    stream_handle: GraphicsStreamHandle,
    texture_handle: GraphicsTextureHandle,
}

impl SpectatorSelectionRender {
    pub fn new(graphics: &Graphics, creator: &UiCreator) -> Self {
        let mut ui = UiContainer::new(creator);
        ui.set_main_panel_color(&Color32::TRANSPARENT);
        Self {
            ui,
            spectator_selection_ui: SpectatorSelectionUi::new(),

            backend_handle: graphics.backend_handle.clone(),
            canvas_handle: graphics.canvas_handle.clone(),
            stream_handle: graphics.stream_handle.clone(),
            texture_handle: graphics.texture_handle.clone(),
        }
    }

    pub fn render(
        &mut self,
        pipe: &mut SpectatorSelectionRenderPipe,
    ) -> VecDeque<SpectatorSelectionEvent> {
        let mut events: VecDeque<SpectatorSelectionEvent> = Default::default();
        let window_width = self.canvas_handle.window_width();
        let window_height = self.canvas_handle.window_height();
        let window_pixels_per_point = self.canvas_handle.window_pixels_per_point();

        let mut user_data = UserData {
            skin_container: pipe.skin_container,
            skin_renderer: pipe.skin_renderer,
            character_infos: pipe.character_infos,
            canvas_handle: &self.canvas_handle,
            stream_handle: &self.stream_handle,
            events: &mut events,
        };
        let mut dummy_pipe = UiRenderPipe::new(*pipe.cur_time, &mut user_data);
        let (screen_rect, full_output, zoom_level) = self.ui.render_cached(
            window_width,
            window_height,
            window_pixels_per_point,
            |ui, inner_pipe, ui_state| self.spectator_selection_ui.render(ui, inner_pipe, ui_state),
            &mut dummy_pipe,
            pipe.input.take().unwrap_or_default(),
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
        events
    }
}
