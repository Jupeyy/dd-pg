use client_types::chat::{ChatMsg, ChatMsgPlayerChannel};
use pool::datatypes::StringPool;
use shared::game::state_wasm_manager::GameStateWasmManager;
use shared_base::network::types::chat::NetChatMsg;
use shared_game::state::state::GameStateInterface;

pub fn from_net_msg(game: &GameStateWasmManager, msg: NetChatMsg, pool: &StringPool) -> ChatMsg {
    let chat_info = game.collect_player_chat_info(&msg.player_id);
    ChatMsg {
        player: chat_info.player_name.clone(),
        skin_name: chat_info.skin_name.clone(),
        msg: msg.msg,
        channel: ChatMsgPlayerChannel::from_net_msg(msg.channel, pool),
    }
}
