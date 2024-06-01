use std::{
    collections::VecDeque,
    sync::{atomic::AtomicBool, Arc},
    time::Duration,
};

use async_trait::async_trait;
use base::system::SystemTime;
use network::network::{
    connection::NetworkConnectionID, event::NetworkEvent,
    event_generator::NetworkEventToGameEventGenerator,
};
use tokio::sync::Mutex;

use crate::messages::GameMessage;

pub enum GameEvents {
    NetworkEvent(NetworkEvent),
    NetworkMsg(GameMessage),
}

pub struct GameEventGenerator {
    pub events: Arc<Mutex<VecDeque<(NetworkConnectionID, Duration, GameEvents)>>>,
    pub has_events: Arc<AtomicBool>,
}

impl GameEventGenerator {
    pub fn new(has_events: Arc<AtomicBool>, _sys: Arc<SystemTime>) -> Self {
        GameEventGenerator {
            events: Default::default(),
            has_events: has_events,
        }
    }
}

#[async_trait]
impl NetworkEventToGameEventGenerator for GameEventGenerator {
    async fn generate_from_binary(
        &self,
        timestamp: Duration,
        con_id: &NetworkConnectionID,
        bytes: &[u8],
    ) {
        let msg =
            bincode::serde::decode_from_slice::<GameMessage, _>(bytes, bincode::config::standard());
        if let Ok((msg, _)) = msg {
            self.events
                .lock()
                .await
                .push_back((*con_id, timestamp, GameEvents::NetworkMsg(msg)));
            self.has_events
                .store(true, std::sync::atomic::Ordering::Relaxed);
        }
    }

    async fn generate_from_network_event(
        &self,
        timestamp: Duration,
        con_id: &NetworkConnectionID,
        network_event: &NetworkEvent,
    ) -> bool {
        self.events.lock().await.push_back((
            *con_id,
            timestamp,
            GameEvents::NetworkEvent(network_event.clone()),
        ));
        self.has_events
            .store(true, std::sync::atomic::Ordering::Relaxed);
        true
    }
}
