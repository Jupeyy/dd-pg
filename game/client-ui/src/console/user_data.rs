use client_types::console::ConsoleEntry;
use game_config::config::ConfigGame;

pub struct UserData<'a> {
    pub entries: &'a Vec<ConsoleEntry>,
    pub config_game: &'a mut ConfigGame,
    pub msgs: &'a mut String,
    pub msg: &'a mut String,
    pub select_index: &'a mut Option<usize>,
}
