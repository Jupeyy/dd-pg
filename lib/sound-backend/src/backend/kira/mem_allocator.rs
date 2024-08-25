use anyhow::anyhow;
use hashlink::LinkedHashMap;
use hiarc::{hiarc_safer_arc_mutex, Hiarc};
use kira::sound::static_sound::StaticSoundData;

use sound::sound_mt_types::SoundBackendMemory;

/// allocates memory so the kira backend can interpret it
#[hiarc_safer_arc_mutex]
#[derive(Debug, Hiarc, Default)]
pub struct MemoryAllocator {
    id_gen: u128,

    flushed_sound_memory: LinkedHashMap<u128, StaticSoundData>,
}

#[hiarc_safer_arc_mutex]
impl MemoryAllocator {
    pub fn new() -> Self {
        Self {
            id_gen: 0,
            flushed_sound_memory: Default::default(),
        }
    }

    pub fn mem_alloc(&mut self, size: usize) -> SoundBackendMemory {
        assert!(size > 0, "an allocation of 0 is an implementation bug");
        let id = self.id_gen;
        self.id_gen += 1;
        SoundBackendMemory::Vector {
            data: vec![0; size],
            id,
        }
    }

    pub fn try_flush_mem(&mut self, mem: &mut SoundBackendMemory) -> anyhow::Result<()> {
        match mem {
            SoundBackendMemory::Vector { data, id } => {
                anyhow::ensure!(!data.is_empty(), "sound memory was already taken.");
                let sound_data =
                    StaticSoundData::from_cursor(std::io::Cursor::new(std::mem::take(data)))?;
                self.flushed_sound_memory.insert(*id, sound_data);
            }
        }

        Ok(())
    }

    pub fn sound_data_from_mem(
        &mut self,
        mut mem: SoundBackendMemory,
    ) -> anyhow::Result<StaticSoundData> {
        match &mem {
            SoundBackendMemory::Vector { id, .. } => {
                let id = *id;
                if !self.flushed_sound_memory.contains_key(&id) {
                    self.try_flush_mem(&mut mem)?;
                }
                self.flushed_sound_memory
                    .remove(&id)
                    .ok_or(anyhow!("static sound data could not be created"))
            }
        }
    }
}
