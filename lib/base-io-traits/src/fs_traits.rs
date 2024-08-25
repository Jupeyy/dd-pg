pub use rustc_hash::{FxHashMap as HashMap, FxHashSet as HashSet};
use std::{
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
    /// Read a file from any file system
    async fn read_file(&self, file_path: &Path) -> std::io::Result<Vec<u8>>;

    /// Read a file in a given file system
    async fn read_file_in(
        &self,
        file_path: &Path,
        path: FileSystemPath,
    ) -> std::io::Result<Vec<u8>>;

    /// Does the given file exist in the read-write file system
    async fn file_exists(&self, file_path: &Path) -> bool;
    /// Write a file to the read-write file system
    async fn write_file(&self, file_path: &Path, data: Vec<u8>) -> std::io::Result<()>;
    /// Create a directory recursively to the read-write file system
    async fn create_dir(&self, dir_path: &Path) -> std::io::Result<()>;

    /// Get's the name of all entries in a directory, that also includes directories.
    async fn entries_in_dir(&self, path: &Path) -> anyhow::Result<HashSet<String>>;

    /// Get's all files in a directory recursively.
    /// Additionally this method guarantees that the path
    /// is separated by a slash (`my_dir/my_file.txt`),
    /// so namingly on Windows the paths get converted from a backslash.
    async fn files_in_dir_recursive(
        &self,
        path: &Path,
    ) -> anyhow::Result<HashMap<PathBuf, Vec<u8>>>;

    /// get the path to the directory to which files will be written to
    fn get_save_path(&self) -> PathBuf;

    /// Get the path to the directory that is not part of the config,
    /// but a separate path to store secure stuff like keys.
    fn get_secure_path(&self) -> PathBuf;

    /// The optional parameter `file` specifies if a specific file within the `path` should be watched
    fn watch_for_change(
        &self,
        path: &Path,
        file: Option<&Path>,
    ) -> Box<dyn FileSystemWatcherItemInterface>;
}
