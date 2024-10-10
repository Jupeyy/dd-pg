use std::collections::VecDeque;

use client_containers::{ninja::NinjaContainer, skins::SkinContainer, weapons::WeaponContainer};
use client_render_base::render::{tee::RenderTee, toolkit::ToolkitRender};
use client_types::actionfeed::ActionInFeed;
use game_interface::types::{character_info::NetworkSkinInfo, resource_key::ResourceKey};
use graphics::handles::{
    canvas::canvas::GraphicsCanvasHandle, stream::stream::GraphicsStreamHandle,
};
use math::math::vector::vec2;

pub struct RenderTeeInfo {
    pub skin: ResourceKey,
    pub skin_info: NetworkSkinInfo,
    pub pos: vec2,
}

pub struct UserData<'a> {
    pub entries: &'a VecDeque<ActionInFeed>,
    pub stream_handle: &'a GraphicsStreamHandle,
    pub canvas_handle: &'a GraphicsCanvasHandle,
    pub skin_container: &'a mut SkinContainer,
    pub render_tee: &'a RenderTee,
    pub weapon_container: &'a mut WeaponContainer,
    pub toolkit_render: &'a ToolkitRender,
    pub ninja_container: &'a mut NinjaContainer,

    pub render_tee_helper: &'a mut Vec<RenderTeeInfo>,
}
