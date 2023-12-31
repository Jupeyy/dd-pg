use hashlink::LinkedHashMap;
use native::input::binds::Binds;
use shared_base::binds::BindActionsLocalPlayer;
use shared_base::game_types::TGameElementID;
use shared_game::player::player::PlayerInput;
use ui_wasm_manager::UIWinitWrapper;

#[derive(Default)]
pub struct ClientPlayer {
    pub input: PlayerInput,
    pub sent_input: PlayerInput,
    pub binds: Binds<BindActionsLocalPlayer>,

    pub chat_input_active: bool,

    pub chat_msg: String,
    pub chat_state: Option<UIWinitWrapper>,

    pub show_scoreboard: bool,
}

pub type LocalPlayers = LinkedHashMap<TGameElementID, ClientPlayer>;
