use std::{collections::VecDeque, rc::Rc, thread::ThreadId};

use api::{graphics::graphics::GraphicsBackend, println, GRAPHICS, IO, RUNTIME_THREAD_POOL};
use base_io::{io_batcher::IoBatcherTask, yield_now::yield_now};
use client_containers::skins::SkinContainer;
use client_types::{
    chat::{ChatMsg, ChatMsgPlayerChannel, ServerMsg},
    console::{ConsoleEntry, ConsoleEntryCmd},
    server_browser::{ServerBrowserData, ServerBrowserFilter, ServerBrowserServer},
};
use client_ui::console::user_data::UserData;
use graphics::graphics::graphics::Graphics;
use ui_base::types::{UiRenderPipe, UiState};
use ui_traits::traits::UiPageInterface;

pub struct Console {}

impl Default for Console {
    fn default() -> Self {
        Self::new()
    }
}

impl Console {
    pub fn new() -> Self {
        Self {}
    }

    fn render_impl(
        &mut self,
        ui: &mut egui::Ui,
        pipe: &mut UiRenderPipe<()>,
        ui_state: &mut UiState,
        main_frame_only: bool,
    ) {
        let mut logs = String::new();
        for i in 0..100 {
            logs.push_str(&format!("test {i}\ntestr2\n"));
        }
        client_ui::console::main_frame::render(
            ui,
            &mut UiRenderPipe {
                cur_time: pipe.cur_time,
                user_data: &mut UserData {
                    entries: &vec![
                        ConsoleEntry::Cmd(ConsoleEntryCmd {
                            name: "test".to_string(),
                            usage: "test".to_string(),
                            cmd: Rc::new(|_, _, _| Ok(())),
                            args: vec![],
                        }),
                        ConsoleEntry::Cmd(ConsoleEntryCmd {
                            name: "test2".to_string(),
                            usage: "test2".to_string(),
                            cmd: Rc::new(|_, _, _| Ok(())),
                            args: vec![],
                        }),
                        ConsoleEntry::Cmd(ConsoleEntryCmd {
                            name: "test3".to_string(),
                            usage: "test3".to_string(),
                            cmd: Rc::new(|_, _, _| Ok(())),
                            args: vec![],
                        }),
                    ],
                    config: &mut Default::default(),
                    msgs: &mut logs,
                    msg: &mut "te".to_string(),
                    cursor: &mut 0,
                    select_index: &mut Some(0),
                },
            },
            ui_state,
            main_frame_only,
        )
    }
}

impl UiPageInterface<()> for Console {
    fn has_blur(&self) -> bool {
        true
    }

    fn render_main_frame(
        &mut self,
        ui: &mut egui::Ui,
        pipe: &mut UiRenderPipe<()>,
        ui_state: &mut UiState,
    ) {
        self.render_impl(ui, pipe, ui_state, true)
    }

    fn render(&mut self, ui: &mut egui::Ui, pipe: &mut UiRenderPipe<()>, ui_state: &mut UiState) {
        self.render_impl(ui, pipe, ui_state, false)
    }
}
