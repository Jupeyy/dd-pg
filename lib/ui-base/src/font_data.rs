use std::sync::Arc;

use base_io::{io::IO, io_batcher::IOBatcherTask};
use hiarc::Hiarc;
use serde::{Deserialize, Serialize};

/// Loading process of shared font data
pub struct UiFontDataLoading {
    task: IOBatcherTask<UiFontData>,
}

impl UiFontDataLoading {
    pub fn new(io: &IO) -> Self {
        let fs = io.fs.clone();
        let task = io.io_batcher.spawn(async move {
            let icon = fs.open_file("fonts/Icons.otf".as_ref()).await?;
            let latin = fs.open_file("fonts/DejaVuSans.ttf".as_ref()).await?;

            Ok(UiFontData { icon, latin })
        });

        Self { task }
    }
}

/// Font data that can (and maybe should) be shared
/// across multiple ui instances over your the application
#[derive(Debug, Hiarc, Serialize, Deserialize)]
pub struct UiFontData {
    pub icon: Vec<u8>,
    pub latin: Vec<u8>,
}

impl UiFontData {
    pub fn new(loading: UiFontDataLoading) -> anyhow::Result<Arc<Self>> {
        Ok(Arc::new(loading.task.get_storage()?))
    }
}
