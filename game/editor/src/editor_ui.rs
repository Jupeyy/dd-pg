use std::{sync::Arc, time::Duration};

use base_io::io::IO;
use client_render::generic_ui_renderer;
use config::config::ConfigEngine;
use egui::{Color32, FontData, FontDefinitions, FontFamily};
use graphics::{
    graphics::graphics::Graphics,
    handles::{
        backend::backend::GraphicsBackendHandle, canvas::canvas::GraphicsCanvasHandle,
        stream::stream::GraphicsStreamHandle, texture::texture::GraphicsTextureHandle,
    },
};
use ui_base::{font_data::UiFontData, types::UIPipe, ui::UI};

use crate::{
    event::EditorEvent,
    tab::EditorTab,
    tools::{tile_layer::auto_mapper::TileLayerAutoMapper, tool::Tools},
    ui::{
        page::EditorUi,
        user_data::{EditorMenuDialogMode, EditorUiEvent, UserData},
    },
};

pub struct EditorUiRenderPipe<'a> {
    pub cur_time: Duration,
    pub events: &'a mut Vec<EditorEvent>,
    pub config: &'a ConfigEngine,
    pub inp: egui::RawInput,
    pub editor_tab: Option<&'a mut EditorTab>,
    pub ui_events: &'a mut Vec<EditorUiEvent>,
    pub unused_rect: &'a mut Option<egui::Rect>,
    pub tools: &'a mut Tools,
    pub auto_mapper: &'a mut TileLayerAutoMapper,
    pub io: &'a IO,
}

pub struct EditorUiRender {
    pub ui: UI,
    editor_ui: EditorUi,

    menu_dialog_mode: EditorMenuDialogMode,

    backend_handle: GraphicsBackendHandle,
    canvas_handle: GraphicsCanvasHandle,
    stream_handle: GraphicsStreamHandle,
    texture_handle: GraphicsTextureHandle,
}

impl EditorUiRender {
    pub fn new(graphics: &Graphics, shared_fonts: &Arc<UiFontData>) -> Self {
        let mut ui = UI::new(None);
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

        fonts
            .families
            .get_mut(&FontFamily::Monospace)
            .unwrap()
            .push("default_latin".to_owned());

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
            &mut (),
            &mut (),
            &mut UIPipe::new(
                pipe.cur_time,
                &mut UserData {
                    config: pipe.config,
                    editor_tab: pipe.editor_tab,
                    ui_events: pipe.ui_events,

                    canvas_handle: &self.canvas_handle,
                    stream_handle: &self.stream_handle,

                    unused_rect: pipe.unused_rect,

                    menu_dialog_mode: &mut self.menu_dialog_mode,
                    tools: pipe.tools,

                    auto_mapper: pipe.auto_mapper,

                    pointer_is_used: &mut needs_pointer,
                    io: pipe.io,
                },
            ),
            Default::default(),
            pipe.inp,
        )
    }
}
