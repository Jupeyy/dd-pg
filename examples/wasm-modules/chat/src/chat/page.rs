use std::{collections::VecDeque, thread::ThreadId};

use api::{graphics::graphics::GraphicsBackend, println, GRAPHICS, IO, RUNTIME_THREAD_POOL};
use base_io::{io_batcher::IOBatcherTask, yield_now::yield_now};
use client_containers_new::skins::SkinContainer;
use client_types::{
    chat::{ChatMsg, ChatMsgPlayerChannel, ServerMsg},
    console::ConsoleEntry,
    server_browser::{ServerBrowserData, ServerBrowserFilter, ServerBrowserServer},
};
use client_ui::console::user_data::UserData;
use graphics::{
    graphics::graphics::Graphics,
    handles::{canvas::canvas::GraphicsCanvasHandle, stream::stream::GraphicsStreamHandle},
};
use ui_base::types::{UIPipe, UIState};
use ui_traits::traits::UIRenderCallbackFunc;

pub struct ChatPage {
    canvas_handle: GraphicsCanvasHandle,
    stream_handle: GraphicsStreamHandle,
}

impl ChatPage {
    pub fn new(graphics: &Graphics) -> Self {
        Self {
            canvas_handle: graphics.canvas_handle.clone(),
            stream_handle: graphics.stream_handle.clone(),
        }
    }

    fn render_impl(
        &mut self,
        ui: &mut egui::Ui,
        pipe: &mut UIPipe<()>,
        ui_state: &mut UIState,
        main_frame_only: bool,
    ) {
        let mut entries: VecDeque<ServerMsg> = vec![
            ServerMsg::Chat(ChatMsg {
                player: "name".into(),
                clan: "clan".into(),
                skin_name: "skin".into(),
                msg: "test".into(),
                channel: ChatMsgPlayerChannel::Global,
            }),
            ServerMsg::Chat(ChatMsg {
                player: "ngme2".into(),
                clan: "clan2".into(),
                skin_name: "skgn2".into(),
                msg: "WWW a very long message that should hopefully break or smth like that bla bla bla bla bla bla bla bla bla bla bla bla bla bla bla bla".into(),
                channel: ChatMsgPlayerChannel::Global,
            }),
        ].into();
        for _ in 0..20 {
            entries.push_back(
            ServerMsg::Chat(ChatMsg {
                player: "ngme2".into(),
                clan: "clan3".into(),
                skin_name: "skgn2".into(),
                msg: "WWW a very long message that should hopefully break or smth like that bla bla bla bla bla bla bla bla bla bla bla bla bla bla bla bla".into(),
                channel: ChatMsgPlayerChannel::Global,
            }));
        }
        client_ui::chat::main_frame::render(
            ui,
            &mut UIPipe::new(
                pipe.cur_time,
                &mut client_ui::chat::user_data::UserData {
                    entries: &entries,
                    is_chat_show_all: false,
                    is_input_active: false,
                    msg: &mut String::new(),
                    chat_events: &mut Default::default(),
                    canvas_handle: &self.canvas_handle,
                    stream_handle: &self.stream_handle,
                },
            ),
            ui_state,
            main_frame_only,
        );
    }
}

impl UIRenderCallbackFunc<()> for ChatPage {
    fn has_blur(&self) -> bool {
        false
    }

    fn render_main_frame(
        &mut self,
        ui: &mut egui::Ui,
        pipe: &mut UIPipe<()>,
        ui_state: &mut UIState,
    ) {
        self.render_impl(ui, pipe, ui_state, true)
    }

    fn render(&mut self, ui: &mut egui::Ui, pipe: &mut UIPipe<()>, ui_state: &mut UIState) {
        self.render_impl(ui, pipe, ui_state, false)
    }
}
