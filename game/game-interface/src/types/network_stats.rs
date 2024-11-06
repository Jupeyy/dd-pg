use std::time::Duration;

use hiarc::Hiarc;
use serde::{Deserialize, Serialize};

/// The network statistics for a single player.
#[derive(Debug, Hiarc, Default, Clone, Copy, Serialize, Deserialize)]
pub struct PlayerNetworkStats {
    // the estimated RTT of the connection.
    pub ping: Duration,
    // estimated amount of packet loss.
    pub packet_loss: f32,
}
