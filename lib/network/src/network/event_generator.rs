use std::{sync::Arc, time::Duration};

use async_trait::async_trait;

use super::{connection::NetworkConnectionID, event::NetworkEvent, notifier::NetworkEventNotifier};

#[async_trait]
pub trait NetworkEventToGameEventGenerator {
    async fn generate_from_binary(
        &self,
        timestamp: Duration,
        con_id: &NetworkConnectionID,
        bytes: &[u8],
    );

    /// Returns true if the network notifier should be notified
    /// Returning false can make sense if the notifier should not
    /// notify about events with less priority, such as a network stat
    /// event.
    /// Important: You should be careful returning false, it might fill up
    /// your event queue, if you use something like that. E.g. network stats are sent
    /// quite regularly
    async fn generate_from_network_event(
        &self,
        timestamp: Duration,
        con_id: &NetworkConnectionID,
        network_event: &NetworkEvent,
    ) -> bool;
}

#[derive(Clone)]
pub struct InternalGameEventGenerator {
    pub(crate) game_event_generator: Arc<dyn NetworkEventToGameEventGenerator + Sync + Send>,
    pub(crate) game_event_notifier: NetworkEventNotifier,
}

impl InternalGameEventGenerator {
    pub(crate) async fn generate_from_binary(
        &self,
        timestamp: Duration,
        con_id: &NetworkConnectionID,
        bytes: &[u8],
    ) {
        self.game_event_generator
            .generate_from_binary(timestamp, con_id, bytes)
            .await;
        self.game_event_notifier.notify.notify_one();
    }

    pub(crate) async fn generate_from_network_event(
        &self,
        timestamp: Duration,
        con_id: &NetworkConnectionID,
        network_event: &NetworkEvent,
    ) {
        if self
            .game_event_generator
            .generate_from_network_event(timestamp, con_id, network_event)
            .await
        {
            self.game_event_notifier.notify.notify_one();
        }
    }
}
