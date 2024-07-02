use std::{collections::VecDeque, time::Duration};

use client_containers_new::skins::SkinContainer;
use client_render_base::render::tee::RenderTee;
use client_types::chat::ServerMsg;
use client_ui::chat::{
    page::ChatUi,
    user_data::{ChatEvent, UserData},
};
use egui::Color32;
use game_interface::types::game::GameEntityId;
use graphics::{
    graphics::graphics::Graphics,
    handles::{
        backend::backend::GraphicsBackendHandle, canvas::canvas::GraphicsCanvasHandle,
        stream::stream::GraphicsStreamHandle, texture::texture::GraphicsTextureHandle,
    },
};

use ui_base::{
    remember_mut::RememberMut, types::UiRenderPipe, ui::UiContainer, ui_render::render_ui,
};
use ui_traits::traits::UiPageInterface;

pub struct ChatRenderOptions {
    pub is_chat_input_active: bool,
    pub is_chat_show_all: bool,
}

pub struct ChatRenderPipe<'a> {
    pub cur_time: &'a Duration,
    pub options: ChatRenderOptions,
    pub msg: &'a mut String,
    pub input: &'a mut Option<egui::RawInput>,
    pub player_id: &'a GameEntityId,
    pub skin_container: &'a mut SkinContainer,
    pub tee_render: &'a mut RenderTee,
}

pub struct ChatRender {
    pub ui: UiContainer,
    chat_ui: ChatUi,

    pub msgs: RememberMut<VecDeque<ServerMsg>>,

    backend_handle: GraphicsBackendHandle,
    canvas_handle: GraphicsCanvasHandle,
    stream_handle: GraphicsStreamHandle,
    texture_handle: GraphicsTextureHandle,
}

impl ChatRender {
    pub fn new(graphics: &Graphics) -> Self {
        let mut ui = UiContainer::new(None);
        ui.set_main_panel_color(&Color32::TRANSPARENT);
        Self {
            ui,
            chat_ui: ChatUi::new(),

            msgs: Default::default(),

            backend_handle: graphics.backend_handle.clone(),
            canvas_handle: graphics.canvas_handle.clone(),
            stream_handle: graphics.stream_handle.clone(),
            texture_handle: graphics.texture_handle.clone(),
        }
    }

    pub fn render(&mut self, pipe: &mut ChatRenderPipe) -> Vec<ChatEvent> {
        let mut res: Vec<ChatEvent> = Default::default();
        let window_width = self.canvas_handle.window_width();
        let window_height = self.canvas_handle.window_height();
        let window_pixels_per_point = self.canvas_handle.window_pixels_per_point();

        let force_rerender = self.msgs.was_accessed_mut();

        let mut user_data = UserData {
            entries: &self.msgs,
            msg: pipe.msg,
            is_input_active: pipe.options.is_chat_input_active,
            is_chat_show_all: pipe.options.is_chat_show_all,
            chat_events: &mut res,
            canvas_handle: &self.canvas_handle,
            stream_handle: &self.stream_handle,
            skin_container: pipe.skin_container,
            render_tee: pipe.tee_render,
        };
        let mut dummy_pipe = UiRenderPipe::new(*pipe.cur_time, &mut user_data);
        let (screen_rect, full_output, zoom_level) = self.ui.render_cached(
            window_width,
            window_height,
            window_pixels_per_point,
            |ui, inner_pipe, ui_state| self.chat_ui.render(ui, inner_pipe, ui_state),
            &mut dummy_pipe,
            pipe.input.clone().unwrap_or_default(),
            false,
            force_rerender,
        );
        let platform_output = render_ui(
            &mut self.ui,
            full_output,
            &screen_rect,
            zoom_level,
            &self.backend_handle,
            &self.texture_handle,
            &self.stream_handle,
            false,
        );
        res.push(ChatEvent::PlatformOutput(platform_output));
        res
    }
}
