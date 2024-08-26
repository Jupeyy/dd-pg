use std::{sync::Arc, time::Duration};

use base_io::io::Io;
use client_render::generic_ui_renderer;
use config::config::ConfigEngine;
use egui::{Color32, FontData, FontDefinitions, FontFamily, InputState};
use graphics::{
    graphics::graphics::Graphics,
    graphics_mt::GraphicsMultiThreaded,
    handles::{
        backend::backend::GraphicsBackendHandle,
        buffer_object::buffer_object::GraphicsBufferObjectHandle,
        canvas::canvas::GraphicsCanvasHandle, stream::stream::GraphicsStreamHandle,
        texture::texture::GraphicsTextureHandle,
    },
};
use ui_base::{
    font_data::UiFontData,
    types::UiRenderPipe,
    ui::{UiContainer, UiCreator},
};

use crate::{
    tab::EditorTab,
    tools::{tile_layer::auto_mapper::TileLayerAutoMapper, tool::Tools},
    ui::{
        page::EditorUi,
        user_data::{EditorMenuDialogMode, EditorUiEvent, UserData},
    },
    utils::UiCanvasSize,
};

pub struct EditorUiRenderPipe<'a> {
    pub cur_time: Duration,
    pub config: &'a ConfigEngine,
    pub inp: egui::RawInput,
    pub editor_tab: Option<&'a mut EditorTab>,
    pub ui_events: &'a mut Vec<EditorUiEvent>,
    pub unused_rect: &'a mut Option<egui::Rect>,
    pub input_state: &'a mut Option<InputState>,
    pub canvas_size: &'a mut Option<UiCanvasSize>,
    pub tools: &'a mut Tools,
    pub auto_mapper: &'a mut TileLayerAutoMapper,
    pub io: &'a Io,
}

pub struct EditorUiRender {
    pub ui: UiContainer,
    editor_ui: EditorUi,

    menu_dialog_mode: EditorMenuDialogMode,

    backend_handle: GraphicsBackendHandle,
    canvas_handle: GraphicsCanvasHandle,
    stream_handle: GraphicsStreamHandle,
    texture_handle: GraphicsTextureHandle,
    buffer_object_handle: GraphicsBufferObjectHandle,
    graphics_mt: GraphicsMultiThreaded,

    tp: Arc<rayon::ThreadPool>,
}

impl EditorUiRender {
    pub fn new(
        graphics: &Graphics,
        tp: Arc<rayon::ThreadPool>,
        shared_fonts: &Arc<UiFontData>,
        creator: &UiCreator,
    ) -> Self {
        let mut ui = UiContainer::new(None, creator);
        ui.set_main_panel_color(&Color32::TRANSPARENT);

        let mut fonts = FontDefinitions::default();
        fonts.font_data.insert(
            "default_latin".to_owned(),
            FontData::from_owned(shared_fonts.latin.clone()),
        );
        fonts
            .families
            .get_mut(&FontFamily::Proportional)
            .unwrap()
            .insert(0, "default_latin".to_owned());

        fonts.font_data.insert(
            "icons".to_owned(),
            FontData::from_owned(shared_fonts.icon.clone()),
        );
        fonts
            .families
            .insert(FontFamily::Name("icons".into()), vec!["icons".into()]);

        ui.context.egui_ctx.set_fonts(fonts.clone());
        ui.stencil_context.egui_ctx.set_fonts(fonts);

        Self {
            ui,
            editor_ui: EditorUi::new(),

            menu_dialog_mode: EditorMenuDialogMode::None,

            backend_handle: graphics.backend_handle.clone(),
            canvas_handle: graphics.canvas_handle.clone(),
            stream_handle: graphics.stream_handle.clone(),
            texture_handle: graphics.texture_handle.clone(),
            buffer_object_handle: graphics.buffer_object_handle.clone(),
            graphics_mt: graphics.get_graphics_mt(),

            tp,
        }
    }

    pub fn render(&mut self, pipe: EditorUiRenderPipe) -> egui::PlatformOutput {
        let mut needs_pointer = false;
        generic_ui_renderer::render(
            &self.backend_handle,
            &self.texture_handle,
            &self.stream_handle,
            &self.canvas_handle,
            &mut self.ui,
            &mut self.editor_ui,
            &mut UiRenderPipe::new(
                pipe.cur_time,
                &mut UserData {
                    config: pipe.config,
                    editor_tab: pipe.editor_tab,
                    ui_events: pipe.ui_events,

                    canvas_handle: &self.canvas_handle,
                    stream_handle: &self.stream_handle,

                    unused_rect: pipe.unused_rect,
                    input_state: pipe.input_state,
                    canvas_size: pipe.canvas_size,

                    menu_dialog_mode: &mut self.menu_dialog_mode,
                    tools: pipe.tools,

                    auto_mapper: pipe.auto_mapper,

                    pointer_is_used: &mut needs_pointer,
                    io: pipe.io,

                    tp: &self.tp,
                    graphics_mt: &self.graphics_mt,
                    buffer_object_handle: &self.buffer_object_handle,
                    backend_handle: &self.backend_handle,
                },
            ),
            Default::default(),
            pipe.inp,
        )
    }
}
