use std::collections::VecDeque;

use client_containers_new::skins::SkinContainer;
use client_render_base::render::tee::RenderTee;
use client_types::chat::ServerMsg;
use graphics::handles::{
    canvas::canvas::GraphicsCanvasHandle, stream::stream::GraphicsStreamHandle,
};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub enum ChatEvent {
    CurMsg(String),
    MsgSend(String),
    ChatClosed,
    PlatformOutput(egui::PlatformOutput),
}

pub struct UserData<'a> {
    pub entries: &'a VecDeque<ServerMsg>,
    pub msg: &'a mut String,
    pub is_input_active: bool,
    pub is_chat_show_all: bool,
    pub chat_events: &'a mut Vec<ChatEvent>,
    pub stream_handle: &'a GraphicsStreamHandle,
    pub canvas_handle: &'a GraphicsCanvasHandle,
    pub skin_container: &'a mut SkinContainer,
    pub render_tee: &'a RenderTee,
}
