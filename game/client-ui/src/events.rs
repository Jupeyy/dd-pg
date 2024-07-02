use std::net::SocketAddr;

use hiarc::{hiarc_safer_rc_refcell, Hiarc};

#[derive(Debug, Hiarc)]
pub enum UiEvent {
    StartDemo {
        name: String,
    },
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
    /// Window settings changed
    WindowChange,
}

#[hiarc_safer_rc_refcell]
#[derive(Debug, Hiarc)]
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
