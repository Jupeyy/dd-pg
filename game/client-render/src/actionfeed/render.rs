use std::{collections::VecDeque, time::Duration};

use client_containers::{skins::SkinContainer, weapons::WeaponContainer};
use client_render_base::render::{tee::RenderTee, toolkit::ToolkitRender};
use client_types::actionfeed::ActionInFeed;
use client_ui::actionfeed::{page::ActionFeedUi, user_data::UserData};
use egui::Color32;
use graphics::{
    graphics::graphics::Graphics,
    handles::{
        backend::backend::GraphicsBackendHandle, canvas::canvas::GraphicsCanvasHandle,
        stream::stream::GraphicsStreamHandle, texture::texture::GraphicsTextureHandle,
    },
};
use ui_traits::traits::UiPageInterface;

use ui_base::{
    remember_mut::RememberMut,
    types::UiRenderPipe,
    ui::{UiContainer, UiCreator},
    ui_render::render_ui,
};

pub struct ActionfeedRenderPipe<'a> {
    pub cur_time: &'a Duration,
    pub skin_container: &'a mut SkinContainer,
    pub tee_render: &'a mut RenderTee,
    pub weapon_container: &'a mut WeaponContainer,
    pub toolkit_render: &'a ToolkitRender,
}

pub struct ActionfeedRender {
    ui: UiContainer,
    feed_ui: ActionFeedUi,

    pub msgs: RememberMut<VecDeque<ActionInFeed>>,

    backend_handle: GraphicsBackendHandle,
    canvas_handle: GraphicsCanvasHandle,
    stream_handle: GraphicsStreamHandle,
    texture_handle: GraphicsTextureHandle,
}

impl ActionfeedRender {
    pub fn new(graphics: &Graphics, creator: &UiCreator) -> Self {
        let mut ui = UiContainer::new(None, creator);
        ui.set_main_panel_color(&Color32::TRANSPARENT);
        Self {
            ui,
            feed_ui: ActionFeedUi::new(),

            msgs: Default::default(),

            backend_handle: graphics.backend_handle.clone(),
            canvas_handle: graphics.canvas_handle.clone(),
            stream_handle: graphics.stream_handle.clone(),
            texture_handle: graphics.texture_handle.clone(),
        }
    }

    pub fn render(&mut self, pipe: &mut ActionfeedRenderPipe) {
        if self.msgs.len() > 30 {
            self.msgs.truncate(20);
        }

        let window_width = self.canvas_handle.window_width();
        let window_height = self.canvas_handle.window_height();
        let window_pixels_per_point = self.canvas_handle.window_pixels_per_point();

        let force_rerender = self.msgs.was_accessed_mut();

        let mut user_data = UserData {
            entries: &self.msgs,
            stream_handle: &self.stream_handle,
            canvas_handle: &self.canvas_handle,
            skin_container: pipe.skin_container,
            render_tee: pipe.tee_render,
            weapon_container: pipe.weapon_container,
            toolkit_render: pipe.toolkit_render,
        };
        let mut inner_pipe = UiRenderPipe::new(*pipe.cur_time, &mut user_data);
        let (screen_rect, full_output, zoom_level) = self.ui.render_cached(
            window_width,
            window_height,
            window_pixels_per_point,
            |ui, inner_pipe, ui_state| self.feed_ui.render(ui, inner_pipe, ui_state),
            &mut inner_pipe,
            Default::default(),
            false,
            force_rerender,
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
