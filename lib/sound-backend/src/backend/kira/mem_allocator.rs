use anyhow::anyhow;
use hashlink::LinkedHashMap;
use hiarc::{hiarc_safer_arc_mutex, Hiarc};
use kira::sound::static_sound::StaticSoundData;

use sound::sound_mt_types::SoundBackendMemory;

#[hiarc_safer_arc_mutex]
#[derive(Debug, Hiarc, Default)]
struct MemoryAllocatorInner {
    id_gen: u128,

    flushed_sound_memory: LinkedHashMap<u128, StaticSoundData>,
}

#[hiarc_safer_arc_mutex]
impl MemoryAllocatorInner {
    pub fn next_id(&mut self) -> u128 {
        let id = self.id_gen;
        self.id_gen += 1;
        id
    }

    pub fn add(&mut self, id: u128, data: StaticSoundData) {
        self.flushed_sound_memory.insert(id, data);
    }

    pub fn remove(&mut self, id: &u128) -> Option<StaticSoundData> {
        self.flushed_sound_memory.remove(id)
    }

    pub fn contains(&mut self, id: &u128) -> bool {
        self.flushed_sound_memory.contains_key(id)
    }
}

/// allocates memory so the kira backend can interpret it
#[derive(Debug, Hiarc, Default, Clone)]
pub struct MemoryAllocator {
    inner: MemoryAllocatorInner,
}

impl MemoryAllocator {
    pub fn mem_alloc(&self, size: usize) -> SoundBackendMemory {
        assert!(size > 0, "an allocation of 0 is an implementation bug");
        let id = self.inner.next_id();
        SoundBackendMemory::Vector {
            data: vec![0; size],
            id,
        }
    }

    pub fn try_flush_mem(&self, mem: &mut SoundBackendMemory) -> anyhow::Result<()> {
        match mem {
            SoundBackendMemory::Vector { data, id } => {
                anyhow::ensure!(!data.is_empty(), "sound memory was already taken.");
                let sound_data =
                    StaticSoundData::from_cursor(std::io::Cursor::new(std::mem::take(data)))?;
                self.inner.add(*id, sound_data);
            }
        }

        Ok(())
    }

    pub fn sound_data_from_mem(
        &self,
        mut mem: SoundBackendMemory,
    ) -> anyhow::Result<StaticSoundData> {
        match &mem {
            SoundBackendMemory::Vector { id, .. } => {
                let id = *id;
                if !self.inner.contains(&id) {
                    self.try_flush_mem(&mut mem)?;
                }
                self.inner
                    .remove(&id)
                    .ok_or(anyhow!("static sound data could not be created"))
            }
        }
    }
}
