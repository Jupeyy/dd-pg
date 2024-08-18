use super::{server_info::GameServerInfo, server_players::ServerPlayers, votes::Votes};

pub struct UserData<'a> {
    pub browser_menu: crate::main_menu::user_data::UserData<'a>,
    pub server_players: &'a ServerPlayers,
    pub votes: &'a Votes,
    pub game_server_info: &'a GameServerInfo,
}
