use std::collections::VecDeque;

use client_containers::skins::SkinContainer;
use client_render_base::render::tee::RenderTee;
use game_interface::types::{id_types::CharacterId, render::character::CharacterInfo};
use graphics::handles::{
    canvas::canvas::GraphicsCanvasHandle, stream::stream::GraphicsStreamHandle,
};
use hashlink::LinkedHashMap;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SpectatorSelectionEvent {
    FreeView,
    Selected(CharacterId),
}

pub struct UserData<'a> {
    pub canvas_handle: &'a GraphicsCanvasHandle,
    pub stream_handle: &'a GraphicsStreamHandle,
    pub skin_container: &'a mut SkinContainer,
    pub skin_renderer: &'a RenderTee,
    pub character_infos: &'a LinkedHashMap<CharacterId, CharacterInfo>,
    pub events: &'a mut VecDeque<SpectatorSelectionEvent>,
}
