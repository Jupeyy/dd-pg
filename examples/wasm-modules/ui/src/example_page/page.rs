use std::{collections::VecDeque, thread::ThreadId};

use api::{graphics::graphics::GraphicsBackend, println, GRAPHICS, IO, RUNTIME_THREAD_POOL};
use base_io::{io_batcher::IoBatcherTask, yield_now::yield_now};
use client_containers::skins::SkinContainer;
use client_types::{
    chat::{ChatMsg, ChatMsgPlayerChannel, ServerMsg},
    console::ConsoleEntry,
};
use client_ui::console::user_data::UserData;
use game_config::config::Config;
use graphics::graphics::graphics::Graphics;
use ui_base::types::{UiRenderPipe, UiState};
use ui_traits::traits::UiPageInterface;

pub struct ExamplePage {}

impl Default for ExamplePage {
    fn default() -> Self {
        Self::new()
    }
}

impl ExamplePage {
    pub fn new() -> Self {
        Self {}
    }

    fn render_impl(
        &mut self,
        ui: &mut egui::Ui,
        pipe: &mut UiRenderPipe<Config>,
        ui_state: &mut UiState,
        main_frame_only: bool,
    ) {
        super::main_frame::render(ui, pipe, ui_state)
    }
}

impl UiPageInterface<Config> for ExamplePage {
    fn has_blur(&self) -> bool {
        false
    }

    fn render_main_frame(
        &mut self,
        ui: &mut egui::Ui,
        pipe: &mut UiRenderPipe<Config>,
        ui_state: &mut UiState,
    ) {
        self.render_impl(ui, pipe, ui_state, true)
    }

    fn render(
        &mut self,
        ui: &mut egui::Ui,
        pipe: &mut UiRenderPipe<Config>,
        ui_state: &mut UiState,
    ) {
        self.render_impl(ui, pipe, ui_state, false)
    }
}
