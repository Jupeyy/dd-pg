use std::{cell::RefCell, collections::VecDeque};

use base::system::{self, SystemTimeInterface};
use client_containers::skins::SkinContainer;
use client_render_base::render::tee::RenderTee;
use client_types::chat::ServerMsg;
use client_ui::chat::{
    page::ChatUI,
    user_data::{ChatInterface, UserData},
};
use config::config::ConfigEngine;
use egui::Color32;
use graphics::graphics::Graphics;

use network::network::{network::NetworkInOrderChannel, quinn_network::QuinnNetwork};
use shared_base::{
    game_types::TGameElementID,
    network::messages::{MsgClChatMsg, NetworkStr},
};
use shared_network::messages::{ClientToServerMessage, ClientToServerPlayerMessage, GameMessage};
use ui_base::{
    types::{UIFeedbackInterface, UINativePipe, UINativeState, UIPipe, UIRawInputGenerator},
    ui::{UIDummyState, UI},
    ui_render::render_ui_2,
};
use ui_traits::traits::UIRenderCallbackFunc;
use ui_wasm_manager::UIWinitWrapper;

pub struct ChatRenderOptions<'a> {
    pub is_chat_input_active: &'a mut bool,
    pub is_chat_show_all: bool,
}

pub struct ChatRenderPipe<'a> {
    pub graphics: &'a mut Graphics,
    pub sys: &'a system::System,
    pub config: &'a mut ConfigEngine,
    pub msgs: &'a VecDeque<ServerMsg>,
    pub options: ChatRenderOptions<'a>,
    pub msg: &'a mut String,
    pub ui_pipe: &'a mut Option<UIWinitWrapper>,
    pub window: &'a winit::window::Window,
    pub network: &'a mut QuinnNetwork,
    pub player_id: &'a TGameElementID,
    pub skin_container: &'a mut SkinContainer,
    pub tee_render: &'a mut RenderTee,
}

pub struct ClientStatsUIFeedbackDummy {}

impl UIFeedbackInterface for ClientStatsUIFeedbackDummy {}

pub struct ChatRender {
    pub ui: UI<UIDummyState>,
    chat_ui: ChatUI,
}

pub struct UIDummyRawInputGenerator2<'a> {
    state: RefCell<&'a mut Option<UIWinitWrapper>>,
    window: &'a winit::window::Window,
}

pub struct ChatInteraction<'a> {
    network: &'a mut QuinnNetwork,
    player_id: &'a TGameElementID,
}
impl<'a> ChatInterface for ChatInteraction<'a> {
    fn on_message(&mut self, msg: String) {
        self.network.send_in_order_to_server(
            &GameMessage::ClientToServer(ClientToServerMessage::PlayerMsg((
                self.player_id.clone(),
                ClientToServerPlayerMessage::Chat(MsgClChatMsg::Global {
                    msg: NetworkStr::from(&msg).unwrap(),
                }),
            ))),
            NetworkInOrderChannel::Global,
        );
    }
}

impl<'a> UIRawInputGenerator<UIDummyState> for UIDummyRawInputGenerator2<'a> {
    fn get_raw_input(&self, _state: &mut UINativeState<UIDummyState>) -> egui::RawInput {
        if let Some(state) = &mut *self.state.borrow_mut() {
            state.state.take_egui_input(self.window)
        } else {
            Default::default()
        }
    }
    fn process_output(
        &self,
        _state: &mut UINativeState<UIDummyState>,
        ctx: &egui::Context,
        output: egui::PlatformOutput,
    ) {
        if let Some(state) = &mut *self.state.borrow_mut() {
            state.state.handle_platform_output(self.window, ctx, output)
        }
    }
}

impl ChatRender {
    pub fn new() -> Self {
        let mut ui = UI::new(UIDummyState {}, None);
        ui.set_main_panel_color(&Color32::TRANSPARENT);
        Self {
            ui,
            chat_ui: ChatUI::new(),
        }
    }

    pub fn render(&mut self, pipe: &mut ChatRenderPipe) {
        let window_width = pipe.graphics.canvas_handle.window_width();
        let window_height = pipe.graphics.canvas_handle.window_height();
        let window_pixels_per_point = pipe.graphics.canvas_handle.window_pixels_per_point();

        let mut ui_feedback = ClientStatsUIFeedbackDummy {};
        let mut network_chat = ChatInteraction {
            network: pipe.network,
            player_id: pipe.player_id,
        };
        let mut dummy_pipe = UIPipe::new(
            &mut ui_feedback,
            pipe.sys.time_get_nanoseconds(),
            pipe.config,
            UserData {
                entries: pipe.msgs,
                msg: pipe.msg,
                is_input_active: pipe.options.is_chat_input_active,
                is_chat_show_all: pipe.options.is_chat_show_all,
                chat: &mut network_chat,
            },
        );
        let mut dummy_native_pipe = UINativePipe {
            raw_inp_generator: &UIDummyRawInputGenerator2 {
                state: RefCell::new(&mut pipe.ui_pipe),
                window: pipe.window,
            },
        };
        let (screen_rect, full_output, zoom_level) = self.ui.render(
            window_width,
            window_height,
            window_pixels_per_point,
            |ui, inner_pipe, ui_state| self.chat_ui.render(ui, inner_pipe, ui_state, pipe.graphics),
            &mut dummy_pipe,
            &mut dummy_native_pipe,
            false,
        );
        render_ui_2(
            &mut self.ui,
            &mut dummy_native_pipe,
            pipe.skin_container,
            pipe.tee_render,
            full_output,
            &screen_rect,
            zoom_level,
            &mut pipe.graphics,
            false,
        );
    }
}
