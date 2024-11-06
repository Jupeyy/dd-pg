use std::fmt::Debug;

use hiarc::Hiarc;
use serde::{Deserialize, Serialize};

pub trait SoundBackendMemoryCleaner: Debug + Send + Sync {
    fn destroy(&self, id: u128);
}

/// memory allocated from the backend, it can be flushed to create a sound object
/// related memory instance async to the sound handle.
#[derive(Debug, Hiarc)]
pub enum SoundBackendMemory {
    FlushableVector {
        data: Vec<u8>,
        id: u128,
        #[hiarc_skip_unsafe]
        deallocator: Option<Box<dyn SoundBackendMemoryCleaner>>,
    },
    Vector {
        data: Vec<u8>,
    },
}

impl Drop for SoundBackendMemory {
    fn drop(&mut self) {
        match self {
            SoundBackendMemory::FlushableVector {
                deallocator, id, ..
            } => {
                if let Some(deallocator) = deallocator.take() {
                    deallocator.destroy(*id);
                }
            }
            SoundBackendMemory::Vector { .. } => {
                // nothing to do
            }
        }
    }
}

impl Serialize for SoundBackendMemory {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            SoundBackendMemory::FlushableVector { data, .. } => data.serialize(serializer),
            SoundBackendMemory::Vector { data } => data.serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for SoundBackendMemory {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let data = <Vec<u8>>::deserialize(deserializer)?;
        Ok(Self::Vector { data })
    }
}

impl SoundBackendMemory {
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        match self {
            SoundBackendMemory::FlushableVector { data, .. } => data.as_mut_slice(),
            SoundBackendMemory::Vector { data } => data.as_mut_slice(),
        }
    }
}
