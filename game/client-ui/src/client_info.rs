use hiarc::{hiarc_safer_rc_refcell, Hiarc};

#[hiarc_safer_rc_refcell]
#[derive(Debug, Hiarc, Default)]
pub struct ClientInfo {
    local_player_count: usize,
}

#[hiarc_safer_rc_refcell]
impl ClientInfo {
    pub fn set_local_player_count(&mut self, local_player_count: usize) {
        self.local_player_count = local_player_count;
    }

    pub fn local_player_count(&self) -> usize {
        self.local_player_count
    }
}
