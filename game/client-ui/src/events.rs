use std::net::SocketAddr;

use game_interface::{types::game::GameEntityId, votes::MapVote};
use hiarc::{hiarc_safer_rc_refcell, Hiarc};

#[derive(Debug, Hiarc)]
pub enum UiEvent {
    PlayDemo {
        name: String,
    },
    RecordDemo,
    StartEditor,
    Connect {
        addr: SocketAddr,
    },
    Disconnect,
    ConnectLocalPlayer {
        as_dummy: bool,
    },
    DisconnectLocalPlayer,
    Quit,
    Kill,
    /// Window settings changed
    WindowChange,
    VoteKickPlayer {
        voted_player_id: GameEntityId,
    },
    VoteSpecPlayer {
        voted_player_id: GameEntityId,
    },
    VoteMap {
        voted_map: MapVote,
    },
    VoteMisc,
}

#[hiarc_safer_rc_refcell]
#[derive(Debug, Hiarc, Default)]
pub struct UiEvents {
    events: Vec<UiEvent>,
}

#[hiarc_safer_rc_refcell]
impl UiEvents {
    pub fn new() -> Self {
        Self {
            events: Default::default(),
        }
    }

    pub fn push(&mut self, ev: UiEvent) {
        self.events.push(ev);
    }

    pub fn take(&mut self) -> Vec<UiEvent> {
        std::mem::take(&mut self.events)
    }
}
