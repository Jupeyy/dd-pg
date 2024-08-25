use client_containers::{emoticons::EmoticonsContainer, skins::SkinContainer};
use client_render_base::render::tee::RenderTee;
use game_interface::types::{
    character_info::NetworkSkinInfo, emoticons::EmoticonType, render::character::TeeEye,
    resource_key::ResourceKey,
};
use graphics::handles::{
    canvas::canvas::GraphicsCanvasHandle, stream::stream::GraphicsStreamHandle,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum EmoteWheelEvent {
    EmoticonSelected(EmoticonType),
    EyeSelected(TeeEye),
}

pub struct UserData<'a> {
    pub stream_handle: &'a GraphicsStreamHandle,
    pub canvas_handle: &'a GraphicsCanvasHandle,
    pub skin_container: &'a mut SkinContainer,
    pub emoticons_container: &'a mut EmoticonsContainer,
    pub render_tee: &'a RenderTee,
    pub events: &'a mut Vec<EmoteWheelEvent>,

    pub emoticon: &'a ResourceKey,
    pub skin: &'a ResourceKey,
    pub skin_info: &'a Option<NetworkSkinInfo>,
}
