use client_types::scoreboard::ScoreboardGameType;

pub struct UserData<'a> {
    pub game_data: &'a ScoreboardGameType,
}
