use std::path::{Path, PathBuf};

use account_client::errors::FsLikeError;

#[derive(Debug)]
pub struct Fs {
    pub secure_path: PathBuf,
}

impl Fs {
    async fn create_dirs_impl(path: impl AsRef<Path>) -> anyhow::Result<(), FsLikeError> {
        Ok(tokio::fs::create_dir_all(path).await?)
    }

    pub async fn new(secure_path: PathBuf) -> anyhow::Result<Self, FsLikeError> {
        Self::create_dirs_impl(&secure_path).await?;
        Ok(Self { secure_path })
    }

    pub async fn create_dirs(&self, path: &Path) -> anyhow::Result<(), FsLikeError> {
        Self::create_dirs_impl(self.secure_path.join(path)).await
    }

    pub async fn write(&self, path: &Path, file: Vec<u8>) -> anyhow::Result<(), FsLikeError> {
        Ok(tokio::fs::write(self.secure_path.join(path), file).await?)
    }

    pub async fn read(&self, path: &Path) -> anyhow::Result<Vec<u8>, FsLikeError> {
        Ok(tokio::fs::read(self.secure_path.join(path)).await?)
    }

    pub async fn remove(&self, path: &Path) -> anyhow::Result<(), FsLikeError> {
        Ok(tokio::fs::remove_file(self.secure_path.join(path)).await?)
    }
}
