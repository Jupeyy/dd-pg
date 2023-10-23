use std::{cell::RefCell, collections::VecDeque};

use base::system::{self, SystemTimeInterface};
use client_types::chat::ServerMsg;
use config::config::Config;
use egui::{Color32, Layout};
use graphics_backend::types::Graphics;
use graphics_base_traits::traits::GraphicsSizeQuery;
use network::network::{network::NetworkInOrderChannel, quinn_network::QuinnNetwork};
use shared_base::{
    game_types::TGameElementID,
    network::messages::{MsgClChatMsg, NetworkStr},
};
use shared_network::messages::{ClientToServerMessage, ClientToServerPlayerMessage, GameMessage};
use ui_base::{
    types::{
        UIFeedbackInterface, UINativePipe, UINativeState, UIPipe, UIRawInputGenerator, UIState,
    },
    ui::{UIDummyState, UIInterface, UI},
    ui_render::render_ui,
};
use ui_wasm_manager::UIWinitWrapper;

pub struct ChatRenderOptions<'a> {
    pub is_chat_input_active: &'a mut bool,
    pub is_chat_show_all: bool,
}

pub struct ChatRenderPipe<'a> {
    pub graphics: &'a mut Graphics,
    pub sys: &'a system::System,
    pub config: &'a mut Config,
    pub msgs: &'a VecDeque<ServerMsg>,
    pub options: ChatRenderOptions<'a>,
    pub msg: &'a mut String,
    pub ui_pipe: &'a mut Option<UIWinitWrapper>,
    pub window: &'a winit::window::Window,
    pub network: &'a mut QuinnNetwork,
    pub player_id: &'a TGameElementID,
}

pub struct ClientStatsUIFeedbackDummy {}

impl UIFeedbackInterface for ClientStatsUIFeedbackDummy {}

pub struct ChatRender {
    pub ui: UI<UIDummyState>,
}

pub struct UIDummyRawInputGenerator2<'a> {
    state: RefCell<&'a mut Option<UIWinitWrapper>>,
    window: &'a winit::window::Window,
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
        Self { ui }
    }

    pub fn render_ui(
        msgs: &VecDeque<ServerMsg>,
        msg: &mut String,
        ui: &mut egui::Ui,
        _pipe: &mut UIPipe<()>,
        _state: &mut UIState,
        options: &mut ChatRenderOptions,
        network: &mut QuinnNetwork,
        player_id: &TGameElementID,
    ) {
        ui.with_layout(Layout::bottom_up(egui::Align::Min), |ui| {
            // active input comes first (most bottom)
            if *options.is_chat_input_active {
                ui.horizontal(|ui| {
                    ui.label("All:");
                    let label = ui.text_edit_singleline(msg);
                    if label.lost_focus() {
                        *options.is_chat_input_active = false;
                        network.send_in_order_to_server(
                            &GameMessage::ClientToServer(ClientToServerMessage::PlayerMsg((
                                player_id.clone(),
                                ClientToServerPlayerMessage::Chat(MsgClChatMsg::Global {
                                    msg: NetworkStr::from(&msg).unwrap(),
                                }),
                            ))),
                            NetworkInOrderChannel::Global,
                        );
                        msg.clear();
                    } else {
                        label.request_focus();
                    }
                });
            }
            for msg in msgs.iter().rev() {
                match msg {
                    ServerMsg::Chat(msg) => ui.label(&msg.msg),
                    ServerMsg::System(msg) => ui.label(&msg.msg),
                };
            }
        });
    }

    pub fn render(&mut self, pipe: &mut ChatRenderPipe) {
        let window_width = pipe.graphics.window_width();
        let window_height = pipe.graphics.window_height();
        let window_pixels_per_point = pipe.graphics.window_pixels_per_point();

        let mut ui_feedback = ClientStatsUIFeedbackDummy {};
        let mut dummy_pipe = UIPipe::new(
            &mut ui_feedback,
            pipe.sys.time_get_nanoseconds(),
            pipe.config,
            (),
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
            |ui, inner_pipe, ui_state| {
                Self::render_ui(
                    pipe.msgs,
                    pipe.msg,
                    ui,
                    inner_pipe,
                    ui_state,
                    &mut pipe.options,
                    pipe.network,
                    pipe.player_id,
                )
            },
            &mut dummy_pipe,
            &mut dummy_native_pipe,
            false,
        );
        render_ui(
            &mut self.ui,
            &mut dummy_native_pipe,
            full_output,
            &screen_rect,
            zoom_level,
            &mut pipe.graphics,
            false,
        );
    }
}
