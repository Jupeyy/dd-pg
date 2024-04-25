use std::{
    collections::HashMap,
    ffi::OsString,
    fmt::Debug,
    path::{Path, PathBuf},
};

use async_trait::async_trait;

pub const MAX_PATH_LEN: usize = 256;

#[derive(Debug, Clone, Copy)]
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
#[derive(Debug, Clone, Copy)]
pub enum FileSystemPath {
    OfType(FileSystemType),
    Index(usize),
}

pub trait FileSystemWatcherItemInterface {
    fn has_file_change(&self) -> bool;
}

#[async_trait]
pub trait FileSystemInterface: Debug + Sync + Send {
    async fn open_file(&self, file_path: &Path) -> std::io::Result<Vec<u8>>;

    async fn open_file_in(
        &self,
        file_path: &Path,
        path: FileSystemPath,
    ) -> std::io::Result<Vec<u8>>;

    async fn file_exists(&self, file_path: &Path) -> bool;
    async fn write_file(&self, file_path: &Path, data: Vec<u8>) -> std::io::Result<()>;

    async fn create_dir(&self, dir_path: &Path) -> std::io::Result<()>;

    async fn files_of_dir(
        &self,
        path: &Path,
        file_read_cb: &mut (dyn FnMut(OsString, Vec<u8>) + Send + Sync),
    );

    async fn files_in_dir_recursive(
        &self,
        path: &Path,
    ) -> anyhow::Result<HashMap<PathBuf, Vec<u8>>>;

    /// get the path to the directory to which files will be written to
    fn get_save_path(&self) -> PathBuf;

    /// The optional parameter `file` specifies if a specific file within the `path` should be watched
    fn watch_for_change(
        &self,
        path: &Path,
        file: Option<&Path>,
    ) -> Box<dyn FileSystemWatcherItemInterface>;
}
