use std::{collections::HashSet, path::PathBuf, sync::Arc};

use base_io_traits::fs_traits::FileSystemInterface;

/// Automatically create votes for all maps locally stored.
pub struct AutoMapVotes {
    pub map_files: HashSet<PathBuf>,
}

impl AutoMapVotes {
    pub async fn new(fs: &Arc<dyn FileSystemInterface>) -> anyhow::Result<Self> {
        let dir = fs.entries_in_dir("map/maps".as_ref()).await?;

        Ok(Self {
            map_files: dir
                .into_iter()
                .filter_map(|p| p.ends_with(".twmap").then(|| p.try_into().ok()).flatten())
                .collect(),
        })
    }
}
