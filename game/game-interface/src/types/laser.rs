use hiarc::Hiarc;
use serde::{Deserialize, Serialize};

#[derive(Debug, Hiarc, Default, Copy, Clone, Serialize, Deserialize)]
pub enum LaserType {
    #[default]
    Rifle,
    Shotgun, // TODO: rename to puller?
    Door,
    Freeze,
}
