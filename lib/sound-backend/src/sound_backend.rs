use std::sync::Arc;

use config::config::ConfigSound;
use hiarc::Hiarc;

use crate::backend_thread::{SoundBackendMtType, SoundBackendThread};
use sound::{
    backend_types::{SoundBackendInterface, SoundManagerMtInterface},
    commands::SoundCommand,
    sound_mt_types::SoundBackendMemory,
};

#[derive(Debug, Hiarc)]
pub struct SoundBackendMt {
    backend_mt: SoundBackendMtType,
}

impl SoundManagerMtInterface for SoundBackendMt {
    fn mem_alloc(&self, size: usize) -> SoundBackendMemory {
        self.backend_mt.as_ref().mem_alloc(size)
    }

    fn try_flush_mem(&self, mem: &mut SoundBackendMemory) -> anyhow::Result<()> {
        self.backend_mt.as_ref().try_flush_mem(mem)
    }
}

#[derive(Debug, Hiarc)]
pub struct SoundBackend {
    // deinit backend last
    backend: SoundBackendThread,
    backend_mt: Arc<SoundBackendMt>,
}

impl SoundBackend {
    pub fn new(config: &ConfigSound) -> anyhow::Result<Box<Self>> {
        let (backend, backend_mt) = SoundBackendThread::new(config)?;

        Ok(Box::new(Self {
            backend,
            backend_mt: Arc::new(SoundBackendMt { backend_mt }),
        }))
    }

    pub fn get_backend_mt(&self) -> Arc<SoundBackendMt> {
        self.backend_mt.clone()
    }
}

impl SoundBackendInterface for SoundBackend {
    fn run_cmds(&mut self, cmds: Vec<SoundCommand>) {
        self.backend.run_cmds(cmds).unwrap();
    }

    fn get_backend_mt(&self) -> Arc<dyn SoundManagerMtInterface> {
        self.backend_mt.clone()
    }
}
