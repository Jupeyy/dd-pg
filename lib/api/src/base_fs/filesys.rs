use std::sync::atomic::AtomicU64;

use async_trait::async_trait;
use base_io::yield_now;
use base_io_traits::fs_traits::{
    FileSystemInterface, FileSystemPath, FileSystemWatcherItemInterface,
};

use crate::{read_result_from_host, upload_param};

extern "C" {
    fn api_open_file();
}

#[derive(Debug)]
pub struct FileSystem {
    id: AtomicU64,
}

impl FileSystem {
    pub fn new() -> Self {
        Self {
            id: Default::default(),
        }
    }
}

#[async_trait]
impl FileSystemInterface for FileSystem {
    async fn open_file(&self, file_path: &str) -> std::io::Result<Vec<u8>> {
        let mut res;
        let id = self.id.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        loop {
            upload_param(0, file_path.to_string());
            upload_param(1, id);
            unsafe {
                api_open_file();
            }
            res = read_result_from_host::<Option<Result<Vec<u8>, String>>>();
            if res.is_some() {
                break;
            } else {
                yield_now::yield_now().await;
            }
        }
        res.unwrap()
            .map_err(|err| std::io::Error::new(std::io::ErrorKind::NotFound, err))
    }

    async fn open_file_in(
        &self,
        _file_path: &str,
        _path: FileSystemPath,
    ) -> std::io::Result<Vec<u8>> {
        todo!("not implemented")
    }

    async fn write_file(&self, _file_path: &str, _data: Vec<u8>) -> std::io::Result<()> {
        todo!("not implemented")
    }

    async fn create_dir(&self, _dir_path: &str) -> std::io::Result<()> {
        todo!("not implemented")
    }

    async fn files_of_dir(
        &self,
        _path: &str,
        _file_read_cb: &mut (dyn FnMut(String, Vec<u8>) + Send + Sync),
    ) {
        todo!("not implemented")
    }

    async fn files_or_dirs_of_dir(
        &self,
        _path: &str,
        _file_read_cb: &mut (dyn FnMut(String) + Send + Sync),
    ) {
        todo!("not implemented")
    }

    fn watch_for_change(
        &self,
        _path: &str,
        _file: Option<&str>,
    ) -> Box<dyn FileSystemWatcherItemInterface> {
        todo!("not implemented")
    }
}
