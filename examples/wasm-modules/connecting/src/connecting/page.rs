use std::{collections::VecDeque, sync::Arc, thread::ThreadId};

use api::{graphics::graphics::GraphicsBackend, println, GRAPHICS, IO, RUNTIME_THREAD_POOL};
use base_io::{io_batcher::IoBatcherTask, yield_now::yield_now};
use client_containers::skins::SkinContainer;
use client_types::{
    chat::{ChatMsg, ChatMsgPlayerChannel, ServerMsg},
    console::ConsoleEntry,
};
use client_ui::{
    connecting::user_data::{ConnectMode, ConnectModes, UserData},
    events::UiEvents,
};
use game_config::config::Config;
use graphics::graphics::graphics::Graphics;
use shared_base::network::server_info::ServerInfo;
use ui_base::types::{UiRenderPipe, UiState};
use ui_traits::traits::UiPageInterface;

pub struct Connecting {}

impl Default for Connecting {
    fn default() -> Self {
        Self::new()
    }
}

impl Connecting {
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
        client_ui::connecting::main_frame::render(
            ui,
            &mut UiRenderPipe {
                cur_time: pipe.cur_time,
                user_data: &mut UserData {
                    mode: &ConnectMode::new(ConnectModes::Connecting),
                    config: pipe.user_data,
                    events: &UiEvents::new(),
                },
            },
            ui_state,
            main_frame_only,
        );
    }
}

impl UiPageInterface<Config> for Connecting {
    fn has_blur(&self) -> bool {
        true
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
