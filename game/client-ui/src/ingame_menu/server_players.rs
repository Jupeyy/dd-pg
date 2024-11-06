pub mod main_frame;

use game_interface::types::{character_info::NetworkCharacterInfo, id_types::PlayerId};
use hashlink::LinkedHashMap;
use hiarc::{hiarc_safer_rc_refcell, Hiarc};

#[hiarc_safer_rc_refcell]
#[derive(Debug, Hiarc, Default)]
pub struct ServerPlayers {
    player_infos: LinkedHashMap<PlayerId, NetworkCharacterInfo>,
    need_player_infos: bool,
}

#[hiarc_safer_rc_refcell]
impl ServerPlayers {
    pub fn request_player_infos(&mut self) {
        self.need_player_infos = true;
    }

    /// Automatically resets the "need" state, so
    /// another [`Players::request_player_infos`] has to
    /// be called.
    pub fn needs_player_infos(&mut self) -> bool {
        std::mem::replace(&mut self.need_player_infos, false)
    }

    pub fn fill_player_info(
        &mut self,
        player_infos: LinkedHashMap<PlayerId, NetworkCharacterInfo>,
    ) {
        self.player_infos = player_infos;
    }

    pub fn collect(&self) -> LinkedHashMap<PlayerId, NetworkCharacterInfo> {
        self.player_infos.clone()
    }
}
