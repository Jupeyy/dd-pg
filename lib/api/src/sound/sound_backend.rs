use std::sync::Arc;

use anyhow::anyhow;
use sound::{
    backend_types::{SoundBackendInterface, SoundManagerMtInterface},
    commands::SoundCommand,
    sound_mt_types::SoundBackendMemory,
};

use crate::upload_param;

extern "C" {
    fn sound_api_run_cmds();
}

#[derive(Debug)]
pub struct SoundMultiThreaded {}

impl SoundManagerMtInterface for SoundMultiThreaded {
    fn mem_alloc(&self, size: usize) -> SoundBackendMemory {
        assert!(size > 0, "an allocation of zero size is not allowed.");
        SoundBackendMemory::Vector {
            data: vec![0; size],
            // don't collide with real ids from the host
            id: u128::MAX,
        }
    }

    fn try_flush_mem(&self, _mem: &mut SoundBackendMemory) -> anyhow::Result<()> {
        Err(anyhow!(
            "flushing sound memory is not supported by a WASM module (not a bug)"
        ))
    }
}

#[derive(Debug)]
pub struct SoundBackend {}

impl SoundBackendInterface for SoundBackend {
    fn run_cmds(&mut self, cmds: Vec<SoundCommand>) {
        upload_param(0, &cmds);
        upload_param(1, true);
        unsafe { sound_api_run_cmds() };
    }

    fn get_backend_mt(&self) -> Arc<dyn SoundManagerMtInterface> {
        Arc::new(SoundMultiThreaded {})
    }
}
