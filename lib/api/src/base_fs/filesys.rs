use async_trait::async_trait;
use base_fs_traits::traits::FileSystemInterface;

use crate::{read_result_from_host, upload_param};

extern "C" {
    fn api_open_file();
}

pub struct FileSystem {}

#[async_trait]
impl FileSystemInterface for FileSystem {
    async fn open_file(&self, file_path: &str) -> std::io::Result<Vec<u8>> {
        upload_param(0, file_path.to_string());
        unsafe {
            api_open_file();
        }
        Ok(read_result_from_host::<Vec<u8>>())
    }
}
