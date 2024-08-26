use std::time::Duration;

use client_containers::skins::SkinContainer;
use client_render_base::render::tee::RenderTee;
use game_interface::{
    types::{character_info::NetworkSkinInfo, resource_key::ResourceKey},
    votes::{MapVote, VoteState, Voted},
};
use graphics::handles::{
    canvas::canvas::GraphicsCanvasHandle, stream::stream::GraphicsStreamHandle,
};

#[derive(Debug, Clone, Copy)]
pub struct VoteRenderPlayer<'a> {
    pub name: &'a str,
    pub skin: &'a ResourceKey,
    pub skin_info: &'a NetworkSkinInfo,
}

#[derive(Debug, Clone, Copy)]
pub enum VoteRenderType<'a> {
    Map(&'a MapVote),
    PlayerVoteKick(VoteRenderPlayer<'a>),
    PlayerVoteSpec(VoteRenderPlayer<'a>),
}

#[derive(Debug, Clone, Copy)]
pub struct VoteRenderData<'a> {
    pub ty: VoteRenderType<'a>,
    pub data: &'a VoteState,
    pub remaining_time: &'a Duration,
    pub voted: Option<Voted>,
}

pub struct UserData<'a> {
    pub stream_handle: &'a GraphicsStreamHandle,
    pub canvas_handle: &'a GraphicsCanvasHandle,
    pub skin_container: &'a mut SkinContainer,
    pub render_tee: &'a RenderTee,

    pub vote_data: VoteRenderData<'a>,
}
