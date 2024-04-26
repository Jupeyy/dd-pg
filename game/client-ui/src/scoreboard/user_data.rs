use game_interface::types::{
    game::GameEntityId,
    render::{character::CharacterInfo, scoreboard::ScoreboardGameType},
};
use graphics::handles::canvas::canvas::GraphicsCanvasHandle;
use hashlink::LinkedHashMap;

pub struct UserData<'a> {
    pub game_data: &'a ScoreboardGameType,
    pub character_infos: &'a LinkedHashMap<GameEntityId, CharacterInfo>,
    pub canvas_handle: &'a GraphicsCanvasHandle,
}
