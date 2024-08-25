use client_containers::skins::SkinContainer;
use client_render_base::render::tee::RenderTee;
use game_interface::types::{
    game::GameEntityId,
    render::{character::CharacterInfo, scoreboard::Scoreboard},
};
use graphics::handles::canvas::canvas::GraphicsCanvasHandle;
use hashlink::LinkedHashMap;

pub struct UserData<'a> {
    pub scoreboard: &'a Scoreboard,
    pub character_infos: &'a LinkedHashMap<GameEntityId, CharacterInfo>,
    pub canvas_handle: &'a GraphicsCanvasHandle,
    pub skin_container: &'a mut SkinContainer,
    pub render_tee: &'a RenderTee,
}
