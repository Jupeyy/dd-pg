use hiarc::Hiarc;
use serde::{Deserialize, Serialize};

/// memory allocated from the backend, it can be flushed to create a sound object
/// related memory instance async to the sound handle.
#[derive(Debug, Hiarc, Serialize, Deserialize)]
pub enum SoundBackendMemory {
    Vector { data: Vec<u8>, id: u128 },
}

impl SoundBackendMemory {
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        match self {
            SoundBackendMemory::Vector { data, .. } => data.as_mut_slice(),
        }
    }
}
