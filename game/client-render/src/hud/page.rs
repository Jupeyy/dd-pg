use std::time::Duration;

use client_containers::{ctf::CtfContainer, skins::SkinContainer};
use client_render_base::render::tee::RenderTee;
use client_ui::hud::{page::HudUi, user_data::UserData};
use egui::Color32;
use game_interface::types::{
    game::{GameEntityId, GameTickType, NonZeroGameTickType},
    render::{character::CharacterInfo, game::GameRenderInfo},
};
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

pub struct HudRenderPipe<'a> {
    pub race_timer_counter: &'a GameTickType,
    pub ticks_per_second: &'a NonZeroGameTickType,
    pub cur_time: &'a Duration,
    pub game: Option<&'a GameRenderInfo>,
    pub skin_container: &'a mut SkinContainer,
    pub skin_renderer: &'a RenderTee,
    pub ctf_container: &'a mut CtfContainer,
    pub character_infos: &'a LinkedHashMap<GameEntityId, CharacterInfo>,
}

pub struct HudRender {
    pub ui: UiContainer,
    hud_ui: HudUi,

    backend_handle: GraphicsBackendHandle,
    canvas_handle: GraphicsCanvasHandle,
    stream_handle: GraphicsStreamHandle,
    texture_handle: GraphicsTextureHandle,
}

impl HudRender {
    pub fn new(graphics: &Graphics, creator: &UiCreator) -> Self {
        let mut ui = UiContainer::new(creator);
        ui.set_main_panel_color(&Color32::TRANSPARENT);
        Self {
            ui,
            hud_ui: HudUi::new(),

            backend_handle: graphics.backend_handle.clone(),
            canvas_handle: graphics.canvas_handle.clone(),
            stream_handle: graphics.stream_handle.clone(),
            texture_handle: graphics.texture_handle.clone(),
        }
    }

    pub fn render(&mut self, pipe: &mut HudRenderPipe) {
        let window_width = self.canvas_handle.window_width();
        let window_height = self.canvas_handle.window_height();
        let window_pixels_per_point = self.canvas_handle.window_pixels_per_point();

        let mut user_data = UserData {
            race_timer_counter: pipe.race_timer_counter,
            ticks_per_second: pipe.ticks_per_second,
            game: pipe.game,
            skin_container: pipe.skin_container,
            skin_renderer: pipe.skin_renderer,
            ctf_container: pipe.ctf_container,
            character_infos: pipe.character_infos,
            canvas_handle: &self.canvas_handle,
            stream_handle: &self.stream_handle,
        };
        let mut dummy_pipe = UiRenderPipe::new(*pipe.cur_time, &mut user_data);
        let (screen_rect, full_output, zoom_level) = self.ui.render_cached(
            window_width,
            window_height,
            window_pixels_per_point,
            |ui, inner_pipe, ui_state| self.hud_ui.render(ui, inner_pipe, ui_state),
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
