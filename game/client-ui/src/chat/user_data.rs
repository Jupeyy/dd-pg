use std::{collections::VecDeque, time::Duration};

use client_containers::skins::SkinContainer;
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

#[derive(Debug)]
pub struct MsgInChat {
    pub msg: ServerMsg,
    pub add_time: Duration,
}

pub struct UserData<'a> {
    pub entries: &'a VecDeque<MsgInChat>,
    pub msg: &'a mut String,
    pub is_input_active: bool,
    pub show_chat_history: bool,
    pub chat_events: &'a mut Vec<ChatEvent>,
    pub stream_handle: &'a GraphicsStreamHandle,
    pub canvas_handle: &'a GraphicsCanvasHandle,
    pub skin_container: &'a mut SkinContainer,
    pub render_tee: &'a RenderTee,
}
