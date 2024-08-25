use hiarc::Hiarc;
use serde::{Deserialize, Serialize};

#[derive(Debug, Hiarc, Default, Copy, Clone, Serialize, Deserialize)]
pub enum FlagType {
    #[default]
    Red,
    Blue,
}
