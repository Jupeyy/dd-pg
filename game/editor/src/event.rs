use std::{
    collections::{HashMap, VecDeque},
    sync::{atomic::AtomicBool, Arc},
    time::Duration,
};

use async_trait::async_trait;
use base::hash::Hash;
use ed25519_dalek::Signature;
use map::map::Map;
use network::network::{
    connection::NetworkConnectionID, event::NetworkEvent,
    event_generator::NetworkEventToGameEventGenerator,
};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use crate::actions::actions::EditorActionGroup;

/// a editor command is the way the user expresses to
/// issue a certain state change.
/// E.g. a undo command means that the server should try to
/// undo the last action.
/// It's basically the logic of the editor ui which does not diretly affect
/// the state of the map.
#[derive(Debug, Serialize, Deserialize)]
pub enum EditorCommand {
    Undo,
    Redo,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EditorEventOverwriteMap {
    pub map: Vec<u8>,
    pub resources: HashMap<Hash, Vec<u8>>,
}

/// editor events are a collection of either actions or commands
#[derive(Debug, Serialize, Deserialize)]
pub enum EditorEvent {
    Action(EditorActionGroup),
    Command(EditorCommand),
    Error(String),
    Auth {
        password: String,
        // if not local user
        is_local_client: bool,
    },
    Map(EditorEventOverwriteMap),
}

pub enum EditorNetEvent {
    Editor(EditorEvent),
    NetworkEvent(NetworkEvent),
}

pub struct EditorEventGenerator {
    pub events: Arc<Mutex<VecDeque<(NetworkConnectionID, Duration, EditorNetEvent)>>>,
    pub has_events: Arc<AtomicBool>,
}

impl EditorEventGenerator {
    pub fn new(has_events: Arc<AtomicBool>) -> Self {
        EditorEventGenerator {
            events: Default::default(),
            has_events,
        }
    }

    pub fn take(&self) -> VecDeque<(NetworkConnectionID, Duration, EditorNetEvent)> {
        std::mem::take(&mut self.events.blocking_lock())
    }
}

#[async_trait]
impl NetworkEventToGameEventGenerator for EditorEventGenerator {
    async fn generate_from_binary(
        &self,
        timestamp: Duration,
        con_id: &NetworkConnectionID,
        bytes: &[u8],
        _signature: Option<Signature>,
    ) {
        let msg =
            bincode::serde::decode_from_slice::<EditorEvent, _>(bytes, bincode::config::standard());
        if let Ok((msg, _)) = msg {
            self.events
                .lock()
                .await
                .push_back((*con_id, timestamp, EditorNetEvent::Editor(msg)));
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
            EditorNetEvent::NetworkEvent(network_event.clone()),
        ));
        self.has_events
            .store(true, std::sync::atomic::Ordering::Relaxed);
        true
    }
}
