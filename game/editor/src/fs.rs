use std::{path::Path, sync::Arc};

use base_io_traits::fs_traits::FileSystemInterface;

/// editor supports global paths, that's why this should be used
pub async fn read_file_editor(
    fs: &Arc<dyn FileSystemInterface>,
    path: &Path,
) -> anyhow::Result<Vec<u8>> {
    if path.is_absolute() {
        Ok(tokio::fs::read(path).await?)
    } else {
        Ok(fs.read_file(path).await?)
    }
}
