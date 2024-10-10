use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize)]
pub struct TickOptions {
    /// Whether this tick should be handled as future tick prediction.
    /// See [`crate::interface::GameStateInterface`] for more information
    /// about prediction code.
    pub is_future_tick_prediction: bool,
}
