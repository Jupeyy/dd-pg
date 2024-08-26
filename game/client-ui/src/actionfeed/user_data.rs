use std::collections::VecDeque;

use client_containers::{skins::SkinContainer, weapons::WeaponContainer};
use client_render_base::render::{tee::RenderTee, toolkit::ToolkitRender};
use client_types::actionfeed::ActionInFeed;
use graphics::handles::{
    canvas::canvas::GraphicsCanvasHandle, stream::stream::GraphicsStreamHandle,
};

pub struct UserData<'a> {
    pub entries: &'a VecDeque<ActionInFeed>,
    pub stream_handle: &'a GraphicsStreamHandle,
    pub canvas_handle: &'a GraphicsCanvasHandle,
    pub skin_container: &'a mut SkinContainer,
    pub render_tee: &'a RenderTee,
    pub weapon_container: &'a mut WeaponContainer,
    pub toolkit_render: &'a ToolkitRender,
}
