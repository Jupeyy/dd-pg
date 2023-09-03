use hashlink::LinkedHashMap;
use native::input::binds::Binds;
use shared_base::binds::BindActions;
use shared_base::game_types::TGameElementID;
use shared_game::player::player::PlayerInput;

#[derive(Default)]
pub struct ClientPlayer {
    pub input: PlayerInput,
    pub sent_input: PlayerInput,
    pub binds: Binds<BindActions>,
}

pub type LocalPlayers = LinkedHashMap<TGameElementID, ClientPlayer>;
