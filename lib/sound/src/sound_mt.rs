use std::{ops::Deref, sync::Arc};

use hiarc::Hiarc;

use crate::backend_types::SoundManagerMtInterface;

#[derive(Debug, Hiarc, Clone)]
pub struct SoundMultiThreaded(#[hiarc_skip_unsafe] pub(crate) Arc<dyn SoundManagerMtInterface>);

impl Deref for SoundMultiThreaded {
    type Target = dyn SoundManagerMtInterface;

    fn deref(&self) -> &Self::Target {
        self.0.as_ref()
    }
}
