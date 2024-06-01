use std::{fmt::Debug, sync::Arc};

use crate::{commands::SoundCommand, sound_mt_types::SoundBackendMemory};

pub trait SoundBackendDriverInterface {
    fn run_cmds(&mut self, cmds: Vec<SoundCommand>) -> anyhow::Result<()>;
}

pub trait SoundBackendMtDriverInterface {
    /// allocate driver memory to write sound files to
    fn mem_alloc(&self, size: usize) -> SoundBackendMemory;
    /// tries to prepare sound files based on the flushed memory
    /// if it fails, that just means it didn't work in a threaded context.
    /// so a fail is to be expected.
    fn try_flush_mem(&self, mem: &mut SoundBackendMemory) -> anyhow::Result<()>;
}

pub trait SoundBackendInterface: Debug {
    fn run_cmds(&mut self, cmds: Vec<SoundCommand>);
    fn get_backend_mt(&self) -> Arc<dyn SoundManagerMtInterface>;
}

pub trait SoundManagerMtInterface: Debug + Sync + Send + 'static {
    /// allocate driver memory to write sound files to
    /// note that sound files in this context really means the raw files (still encoded)
    /// unlike bitmaps usually seen allocating graphics textures
    fn mem_alloc(&self, size: usize) -> SoundBackendMemory;
    /// tries to prepare sound files based on the flushed memory
    /// (as described above, these are the raw encoded sound files)
    /// if it fails, that just means it didn't work in a threaded context (e.g. WASM).
    /// so a fail is to be expected.
    fn try_flush_mem(&self, mem: &mut SoundBackendMemory) -> anyhow::Result<()>;
}
