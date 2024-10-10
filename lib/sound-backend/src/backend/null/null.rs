use std::sync::Arc;

use anyhow::anyhow;
use hiarc::Hiarc;

use sound::{
    backend_types::{SoundBackendDriverInterface, SoundBackendMtDriverInterface},
    commands::SoundCommand,
    frame_fetcher_plugin::BackendFrameFetcher,
    sound_mt_types::SoundBackendMemory,
};

#[derive(Debug, Hiarc)]
pub struct SoundBackendNull {}

impl SoundBackendDriverInterface for SoundBackendNull {
    fn run_cmds(&mut self, _cmds: Vec<SoundCommand>) -> anyhow::Result<()> {
        // nothing to do
        Ok(())
    }

    fn attach_frame_fetcher(&mut self, _name: String, _fetcher: Arc<dyn BackendFrameFetcher>) {}

    fn detach_frame_fetcher(&mut self, _name: String) {}
}

impl SoundBackendMtDriverInterface for SoundBackendNull {
    fn mem_alloc(&self, size: usize) -> SoundBackendMemory {
        SoundBackendMemory::Vector {
            data: vec![0; size],
            id: 0,
        }
    }

    fn try_flush_mem(&self, _mem: &mut SoundBackendMemory) -> anyhow::Result<()> {
        Err(anyhow!("flushing memory is not supported."))
    }
}
