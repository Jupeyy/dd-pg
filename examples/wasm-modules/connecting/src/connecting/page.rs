use std::{collections::VecDeque, sync::Arc, thread::ThreadId};

use api::{graphics::graphics::GraphicsBackend, println, GRAPHICS, IO, RUNTIME_THREAD_POOL};
use base_io::{io_batcher::IOBatcherTask, yield_now::yield_now};
use client_containers::skins::SkinContainer;
use client_types::{
    chat::{ChatMsg, ChatMsgPlayerChannel, ServerMsg},
    console::ConsoleEntry,
    server_browser::{ServerBrowserData, ServerBrowserFilter, ServerBrowserServer},
};
use client_ui::connecting::user_data::{ConnectMode, UserData};
use graphics::graphics::Graphics;
use shared_base::network::server_info::ServerInfo;
use ui_base::types::{UIPipe, UIState};
use ui_traits::traits::UIRenderCallbackFunc;

pub struct Connecting {}

impl Connecting {
    pub fn new() -> Self {
        Self {}
    }

    fn render_impl(
        &mut self,
        ui: &mut egui::Ui,
        pipe: &mut UIPipe<()>,
        ui_state: &mut UIState,
        graphics: &mut Graphics,
        main_frame_only: bool,
    ) {
        client_ui::connecting::main_frame::render(
            ui,
            &mut UIPipe {
                ui_feedback: pipe.ui_feedback,
                cur_time: pipe.cur_time,
                config: pipe.config,
                user_data: UserData {
                    mode: &ConnectMode::Connecting,
                },
            },
            ui_state,
            graphics,
            main_frame_only,
        );
    }
}

impl UIRenderCallbackFunc<()> for Connecting {
    fn has_blur(&self) -> bool {
        true
    }

    fn render_main_frame(
        &mut self,
        ui: &mut egui::Ui,
        pipe: &mut UIPipe<()>,
        ui_state: &mut UIState,
        graphics: &mut Graphics,
    ) {
        self.render_impl(ui, pipe, ui_state, graphics, true)
    }

    fn render(
        &mut self,
        ui: &mut egui::Ui,
        pipe: &mut UIPipe<()>,
        ui_state: &mut UIState,
        graphics: &mut Graphics,
    ) {
        self.render_impl(ui, pipe, ui_state, graphics, false)
    }
}
