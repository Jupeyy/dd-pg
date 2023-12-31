use std::{collections::VecDeque, rc::Rc, thread::ThreadId};

use api::{graphics::graphics::GraphicsBackend, println, GRAPHICS, IO, RUNTIME_THREAD_POOL};
use base_io::{io_batcher::IOBatcherTask, yield_now::yield_now};
use client_containers::skins::SkinContainer;
use client_types::{
    chat::{ChatMsg, ChatMsgPlayerChannel, ServerMsg},
    console::{ConsoleEntry, ConsoleEntryCmd},
    server_browser::{ServerBrowserData, ServerBrowserFilter, ServerBrowserServer},
};
use client_ui::{chat::user_data::ChatInterface, console::user_data::UserData};
use graphics::graphics::Graphics;
use ui_base::types::{UIPipe, UIState};
use ui_traits::traits::UIRenderCallbackFunc;

pub struct Console {}

impl Console {
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
        let mut logs = String::new();
        for i in 0..100 {
            logs.push_str(&format!("test {i}\ntestr2\n"));
        }
        client_ui::console::main_frame::render(
            ui,
            &mut UIPipe {
                ui_feedback: pipe.ui_feedback,
                cur_time: pipe.cur_time,
                config: pipe.config,
                user_data: UserData {
                    entries: &vec![
                        ConsoleEntry::Cmd(ConsoleEntryCmd {
                            name: "test".to_string(),
                            usage: "test".to_string(),
                            cmd: Rc::new(|_, _, _| Ok(())),
                        }),
                        ConsoleEntry::Cmd(ConsoleEntryCmd {
                            name: "test2".to_string(),
                            usage: "test2".to_string(),
                            cmd: Rc::new(|_, _, _| Ok(())),
                        }),
                        ConsoleEntry::Cmd(ConsoleEntryCmd {
                            name: "test3".to_string(),
                            usage: "test3".to_string(),
                            cmd: Rc::new(|_, _, _| Ok(())),
                        }),
                    ],
                    config_game: &mut Default::default(),
                    msgs: &mut logs,
                    msg: &mut "te".to_string(),
                    select_index: &mut Default::default(),
                },
            },
            ui_state,
            graphics,
            main_frame_only,
        )
    }
}

impl UIRenderCallbackFunc<()> for Console {
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
