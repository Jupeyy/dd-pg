use shared_game::types::types::ScoreboardGameType;

pub struct UserData<'a> {
    pub game_data: &'a ScoreboardGameType,
}
