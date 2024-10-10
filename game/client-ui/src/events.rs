use std::{net::SocketAddr, path::PathBuf};

use game_interface::{
    types::{game::GameEntityId, network_string::NetworkReducedAsciiString},
    votes::{MapVote, MiscVote},
};
use hiarc::{hiarc_safer_rc_refcell, Hiarc};
use math::math::vector::ubvec4;

#[derive(Debug, Hiarc)]
pub enum UiEvent {
    PlayDemo {
        name: PathBuf,
    },
    EncodeDemoToVideo {
        name: PathBuf,
        video_name: String,
    },
    RecordDemo,
    StartEditor,
    Connect {
        addr: SocketAddr,
        cert_hash: Option<[u8; 32]>,
        rcon_secret: Option<[u8; 32]>,
    },
    Disconnect,
    ConnectLocalPlayer {
        as_dummy: bool,
    },
    DisconnectLocalPlayer,
    Quit,
    Kill,
    JoinSpectators,
    JoinOwnTeam {
        name: String,
        color: ubvec4,
    },
    JoinOtherTeam,
    JoinVanillaSide {
        is_red_side: bool,
    },
    SwitchToFreeCam,
    /// Window settings changed
    WindowChange,
    VsyncChanged,
    MsaaChanged,
    VoteKickPlayer {
        voted_player_id: GameEntityId,
    },
    VoteSpecPlayer {
        voted_player_id: GameEntityId,
    },
    VoteMap {
        voted_map: MapVote,
    },
    VoteMisc {
        misc: MiscVote,
    },
    ChangeAccountName {
        name: NetworkReducedAsciiString<32>,
    },
    RequestAccountInfo,
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
