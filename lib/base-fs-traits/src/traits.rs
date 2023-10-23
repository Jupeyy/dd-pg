use std::fmt::Debug;

use async_trait::async_trait;

pub const MAX_PATH_LEN: usize = 256;

pub enum FileSystemType {
    // read write has a higher priority, since it contains user modifications
    ReadWrite,
    // the read-only file system is the one shipped with the executables
    Read,
    // working directory
    Exec,
}

// either get the file system path by a specific named type
// or get the path of a specific index
// or allow any path
pub enum FileSystemPath {
    OfType(FileSystemType),
    Index(usize),
}

pub trait FileSystemWatcherItemInterface {
    fn has_file_change(&self) -> bool;
}

#[async_trait]
pub trait FileSystemInterface: Debug + Sync + Send {
    async fn open_file(&self, file_path: &str) -> std::io::Result<Vec<u8>>;

    async fn open_file_in(&self, file_path: &str, path: FileSystemPath)
        -> std::io::Result<Vec<u8>>;

    async fn write_file(&self, file_path: &str, data: Vec<u8>) -> std::io::Result<()>;

    async fn create_dir(&self, dir_path: &str) -> std::io::Result<()>;

    async fn files_of_dir(
        &self,
        path: &str,
        file_read_cb: &mut (dyn FnMut(String, Vec<u8>) + Send + Sync),
    );

    async fn files_or_dirs_of_dir(
        &self,
        path: &str,
        file_read_cb: &mut (dyn FnMut(String) + Send + Sync),
    );

    /**
     * The optional parameter `file` specifies if a specific file within the `path` should be watched
     */
    fn watch_for_change(
        &self,
        path: &str,
        file: Option<&str>,
    ) -> Box<dyn FileSystemWatcherItemInterface>;
}
