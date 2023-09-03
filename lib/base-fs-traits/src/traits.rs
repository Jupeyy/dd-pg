use async_trait::async_trait;

pub const MAX_PATH_LEN: usize = 256;

#[async_trait]
pub trait FileSystemInterface {
    async fn open_file(&self, file_path: &str) -> std::io::Result<Vec<u8>>;
}
