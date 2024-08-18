use std::{collections::VecDeque, thread::ThreadId, time::Duration};

use api::{graphics::graphics::GraphicsBackend, println, GRAPHICS, IO, RUNTIME_THREAD_POOL};
use api_ui_game::render::create_skin_container;
use base_io::{io_batcher::IoBatcherTask, yield_now::yield_now};
use client_containers::skins::SkinContainer;
use client_render_base::render::tee::RenderTee;
use client_types::{
    chat::{ChatMsg, ChatMsgPlayerChannel, ServerMsg},
    console::ConsoleEntry,
    server_browser::{ServerBrowserData, ServerBrowserFilter, ServerBrowserServer},
};
use client_ui::{chat::user_data::MsgInChat, console::user_data::UserData};
use game_interface::types::character_info::NetworkSkinInfo;
use graphics::{
    graphics::graphics::Graphics,
    handles::{canvas::canvas::GraphicsCanvasHandle, stream::stream::GraphicsStreamHandle},
};
use math::math::vector::ubvec4;
use ui_base::types::{UiRenderPipe, UiState};
use ui_traits::traits::UiPageInterface;

pub struct ChatPage {
    canvas_handle: GraphicsCanvasHandle,
    stream_handle: GraphicsStreamHandle,
    skin_container: SkinContainer,
    render_tee: RenderTee,
}

impl ChatPage {
    pub fn new(graphics: &Graphics) -> Self {
        Self {
            canvas_handle: graphics.canvas_handle.clone(),
            stream_handle: graphics.stream_handle.clone(),
            skin_container: create_skin_container(),
            render_tee: RenderTee::new(graphics),
        }
    }

    fn render_impl(
        &mut self,
        ui: &mut egui::Ui,
        pipe: &mut UiRenderPipe<()>,
        ui_state: &mut UiState,
        main_frame_only: bool,
    ) {
        let mut entries: VecDeque<MsgInChat> = vec![
            MsgInChat {
                msg: ServerMsg::Chat(ChatMsg {
                    player: "name".into(),
                    clan: "clan".into(),
                    skin_name: "skin".try_into().unwrap(),
                    skin_info: NetworkSkinInfo::Custom {
                        body_color: ubvec4::new(0, 255, 255, 255),
                        feet_color: ubvec4::new(255, 255, 255, 255),
                    },
                    msg: "test".into(),
                    channel: ChatMsgPlayerChannel::Global,
                }),
                add_time: Duration::ZERO,
            },
            MsgInChat {
                msg: ServerMsg::Chat(ChatMsg {
                    player: "ngme2".into(),
                    clan: "clan2".into(),
                    skin_name: "skgn2".try_into().unwrap(),
                    skin_info: NetworkSkinInfo::Custom {
                        body_color: ubvec4::new(255, 255, 255, 255),
                        feet_color: ubvec4::new(255, 0, 255, 255),
                    },
                    msg: "WWW a very long message that should hopefully break or \
                            smth like that bla bla bla bla bla bla bla bla bla bla \
                            bla bla bla bla bla bla"
                        .into(),
                    channel: ChatMsgPlayerChannel::Global,
                }),
                add_time: Duration::ZERO,
            },
        ]
        .into();
        for _ in 0..20 {
            entries.push_back(MsgInChat {
                msg: ServerMsg::Chat(ChatMsg {
                    player: "ngme2".into(),
                    clan: "clan3".into(),
                    skin_name: "skgn2".try_into().unwrap(),
                    skin_info: NetworkSkinInfo::Original,
                    msg: "WWW a very long message that should hopefully break or \
                            smth like that bla bla bla bla bla bla bla bla bla bla \
                            bla bla bla bla bla bla"
                        .into(),
                    channel: ChatMsgPlayerChannel::Global,
                }),
                add_time: Duration::ZERO,
            });
        }
        client_ui::chat::main_frame::render(
            ui,
            &mut UiRenderPipe::new(
                pipe.cur_time,
                &mut client_ui::chat::user_data::UserData {
                    entries: &entries,
                    show_chat_history: false,
                    is_input_active: false,
                    msg: &mut String::new(),
                    chat_events: &mut Default::default(),
                    canvas_handle: &self.canvas_handle,
                    stream_handle: &self.stream_handle,
                    skin_container: &mut self.skin_container,
                    render_tee: &self.render_tee,
                },
            ),
            ui_state,
            main_frame_only,
        );
    }
}

impl UiPageInterface<()> for ChatPage {
    fn has_blur(&self) -> bool {
        false
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
