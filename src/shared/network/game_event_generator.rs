use std::{
    collections::VecDeque,
    sync::{atomic::AtomicBool, Arc, Condvar},
    time::Duration,
};

use base::system::SystemTime;
use network::network::network::{
    NetworkConnectionID, NetworkEventToGameEventGenerator, NetworkGameEvent,
};

use super::messages::GameMessage;

pub enum GameEvents {
    NetworkEvent(NetworkGameEvent),
    NetworkMsg(GameMessage),
}

pub struct GameEventGenerator {
    pub events: VecDeque<(NetworkConnectionID, Duration, GameEvents)>,
    pub has_events: Arc<AtomicBool>,
    pub ev_cond: Condvar,
}

impl GameEventGenerator {
    pub fn new(has_events: Arc<AtomicBool>, _sys: Arc<SystemTime>) -> Self {
        GameEventGenerator {
            events: VecDeque::new(),
            has_events: has_events,
            ev_cond: Condvar::new(),
        }
    }
}

impl NetworkEventToGameEventGenerator for GameEventGenerator {
    fn generate_from_binary(
        &mut self,
        timestamp: Duration,
        con_id: &NetworkConnectionID,
        bytes: &[u8],
    ) {
        let msg = bincode::decode_from_slice::<GameMessage, _>(bytes, bincode::config::standard());
        if let Ok((msg, _)) = msg {
            self.events
                .push_back((*con_id, timestamp, GameEvents::NetworkMsg(msg)));
            self.has_events
                .store(true, std::sync::atomic::Ordering::Relaxed);
            self.ev_cond.notify_all();
        }
    }

    fn generate_from_network_event(
        &mut self,
        timestamp: Duration,
        con_id: &NetworkConnectionID,
        network_event: &network::network::network::NetworkGameEvent,
    ) {
        self.events.push_back((
            *con_id,
            timestamp,
            GameEvents::NetworkEvent(network_event.clone()),
        ));
        self.has_events
            .store(true, std::sync::atomic::Ordering::Relaxed);
        self.ev_cond.notify_all();
    }
}
