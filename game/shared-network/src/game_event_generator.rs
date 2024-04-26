use std::{
    collections::VecDeque,
    sync::{atomic::AtomicBool, Arc},
    time::Duration,
};

use async_trait::async_trait;
use base::system::SystemTime;
use ed25519_dalek::Signature;
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

pub struct GameEventsSigned {
    pub msg: GameEvents,
    pub signature: Option<(Vec<u8>, Signature)>,
}

pub struct GameEventGenerator {
    pub events: Arc<Mutex<VecDeque<(NetworkConnectionID, Duration, GameEventsSigned)>>>,
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
        signature: Option<Signature>,
    ) {
        let msg =
            bincode::serde::decode_from_slice::<GameMessage, _>(bytes, bincode::config::standard());
        if let Ok((msg, _)) = msg {
            self.events.lock().await.push_back((
                *con_id,
                timestamp,
                GameEventsSigned {
                    msg: GameEvents::NetworkMsg(msg),
                    signature: signature.map(|signature| (bytes.to_vec(), signature)),
                },
            ));
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
            GameEventsSigned {
                msg: GameEvents::NetworkEvent(network_event.clone()),
                signature: None,
            },
        ));
        self.has_events
            .store(true, std::sync::atomic::Ordering::Relaxed);
        true
    }
}
