use client_types::console::ConsoleEntry;
use game_config::config::Config;

pub struct UserData<'a> {
    pub entries: &'a Vec<ConsoleEntry>,
    pub config: &'a mut Config,
    pub msgs: &'a mut String,
    pub msg: &'a mut String,
    pub select_index: &'a mut Option<usize>,
}
