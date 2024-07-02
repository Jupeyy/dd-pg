use std::time::Duration;

use binds::binds::BindActions;
use game_interface::types::game::GameEntityId;
use hashlink::LinkedHashMap;
use native::input::binds::Binds;
use shared_base::player_input::PlayerInput;

#[derive(Debug, Default)]
pub struct ClientPlayer {
    pub input: PlayerInput,
    pub sent_input: PlayerInput,
    pub sent_input_time: Option<Duration>,

    /// last input id the server knows about
    pub server_input_id: Option<u64>,

    pub binds: Binds<Vec<BindActions>>,

    pub chat_input_active: bool,
    pub chat_msg: String,

    pub show_scoreboard: bool,

    // dummy controls
    pub dummy_copy_moves: bool,
    pub dummy_hammer: bool,

    pub is_dummy: bool,
}

pub type LocalPlayers = LinkedHashMap<GameEntityId, ClientPlayer>;
