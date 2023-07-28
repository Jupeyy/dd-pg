use std::{
    collections::HashMap,
    path::Path,
    sync::{atomic::AtomicBool, mpsc::channel, Arc, Mutex, RwLock},
    thread::JoinHandle,
};

use arrayvec::ArrayString;
use base::system::{LogLevel, SystemLog, SystemLogGroup, SystemLogInterface};
use directories::ProjectDirs;
use hashlink::LinkedHashMap;
use notify::{recommended_watcher, INotifyWatcher, RecursiveMode, Watcher};

const MAX_PATH_LEN: usize = 256;

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

struct FileSystemWatcherPath {
    watchers_of_path: Arc<RwLock<LinkedHashMap<usize, Arc<AtomicBool>>>>,
    watcher: Option<INotifyWatcher>,
    thread: Option<JoinHandle<()>>,
    path: String,
    logger: Arc<Mutex<SystemLogGroup>>,
}

impl FileSystemWatcherPath {
    pub fn new(logger: &Arc<Mutex<SystemLogGroup>>, path: &str) -> Self {
        // Create a channel to receive the events.
        let (tx, rx) = channel();

        // Create a watcher object, delivering debounced events.
        // The notification back-end is selected based on the platform.
        let mut watcher = recommended_watcher(tx).unwrap();

        // Add a path to be watched. All files and directories at that path and
        // below will be monitored for changes.
        if let Err(err) = watcher.watch(Path::new(path), RecursiveMode::Recursive) {
            logger
                .lock()
                .unwrap()
                .log(LogLevel::Info)
                .msg("could not watch directory/file: ")
                .msg_var(&err);
        }

        let watchers_of_path: Arc<RwLock<LinkedHashMap<usize, Arc<AtomicBool>>>> =
            Arc::new(RwLock::new(Default::default()));
        let watchers_of_path_thread = watchers_of_path.clone();

        let watch_thread = std::thread::spawn(move || loop {
            match rx.recv() {
                Ok(_) => {
                    watchers_of_path_thread
                        .read()
                        .as_ref()
                        .unwrap()
                        .values()
                        .for_each(|watcher_bool| {
                            watcher_bool.store(true, std::sync::atomic::Ordering::Relaxed)
                        });
                }
                Err(_) => {
                    return;
                }
            }
            ()
        });

        Self {
            watchers_of_path,
            watcher: Some(watcher),
            thread: Some(watch_thread),
            path: path.to_string(),
            logger: logger.clone(),
        }
    }
}

impl Drop for FileSystemWatcherPath {
    fn drop(&mut self) {
        if let Err(err) = self
            .watcher
            .as_mut()
            .unwrap()
            .unwatch(Path::new(&self.path))
        {
            self.logger
                .lock()
                .unwrap()
                .log(LogLevel::Info)
                .msg("could not stop watching directory/file: ")
                .msg_var(&err);
        }

        let mut watcher_swap = None;
        std::mem::swap(&mut watcher_swap, &mut self.watcher);

        drop(watcher_swap.unwrap());

        let mut thread_swap = None;
        std::mem::swap(&mut thread_swap, &mut self.thread);

        thread_swap.unwrap().join().unwrap();
    }
}

#[derive(Default)]
struct FileSystemWatcher {
    path_watchers: HashMap<String, FileSystemWatcherPath>,
    path_watcher_id_generator: usize,
}

pub struct FileSystemWatcherItem {
    fs_watcher: Arc<Mutex<FileSystemWatcher>>,
    path_watcher_id: usize,
    is_changed: Arc<AtomicBool>,
    path: String,
}

impl FileSystemWatcherItem {
    fn new(
        logger: &Arc<Mutex<SystemLogGroup>>,
        path: &str,
        fs_watcher: &Arc<Mutex<FileSystemWatcher>>,
    ) -> Self {
        let mut fs_watcher_write = fs_watcher.lock().unwrap();
        let path_watcher_id = fs_watcher_write.path_watcher_id_generator;
        fs_watcher_write.path_watcher_id_generator += 1;
        let is_changed = Arc::new(AtomicBool::new(false));
        if let Some(path_watcher) = fs_watcher_write.path_watchers.get_mut(path) {
            path_watcher
                .watchers_of_path
                .write()
                .as_mut()
                .unwrap()
                .insert(path_watcher_id, is_changed.clone());
        } else {
            let path_watcher = FileSystemWatcherPath::new(logger, path);
            path_watcher
                .watchers_of_path
                .write()
                .as_mut()
                .unwrap()
                .insert(path_watcher_id, is_changed.clone());
            fs_watcher_write
                .path_watchers
                .insert(path.to_string(), path_watcher);
        }

        Self {
            fs_watcher: fs_watcher.clone(),
            path_watcher_id,
            is_changed,
            path: path.to_string(),
        }
    }

    pub fn has_file_change(&self) -> bool {
        self.is_changed
            .compare_exchange(
                true,
                false,
                std::sync::atomic::Ordering::Relaxed,
                std::sync::atomic::Ordering::Relaxed,
            )
            .unwrap_or(false)
    }
}

impl Drop for FileSystemWatcherItem {
    fn drop(&mut self) {
        let mut fs_watcher_write = self.fs_watcher.lock().unwrap();
        let path_watcher = fs_watcher_write
            .path_watchers
            .get_mut(self.path.as_str())
            .unwrap();
        let mut watchers_of_path_guard = path_watcher.watchers_of_path.write();
        let watchers_of_path = watchers_of_path_guard.as_mut().unwrap();
        watchers_of_path.remove(&self.path_watcher_id);
        let watchers_empty = watchers_of_path.is_empty();
        drop(watchers_of_path_guard);
        if watchers_empty {
            fs_watcher_write.path_watchers.remove(self.path.as_str());
        }
    }
}

pub struct FileSystem {
    pub paths: Vec<String>,
    config_dir_index: usize,
    data_dir_index: usize,
    exec_dir_index: usize,

    fs_watcher: Arc<Mutex<FileSystemWatcher>>,
    _logger: Mutex<SystemLogGroup>,
    logger_fs_watch: Arc<Mutex<SystemLogGroup>>,
}

impl FileSystem {
    pub fn new(log: &SystemLog) -> Self {
        let mut config_dir: String = String::new();
        if let Some(proj_dirs) = ProjectDirs::from("org", "", "DDNet") {
            config_dir = proj_dirs.config_dir().to_str().unwrap().to_string();
            std::fs::DirBuilder::new()
                .recursive(true)
                .create(&config_dir)
                .unwrap();
            if !config_dir.ends_with("/") {
                config_dir.push('/');
            }
        }
        let mut paths: Vec<String> = Vec::new();
        paths.push(config_dir);
        let config_dir_index = paths.len() - 1;
        paths.push("data/".to_string());
        let data_dir_index = paths.len() - 1;
        if let Ok(exec_path) = std::env::current_dir() {
            paths.push(exec_path.to_str().unwrap().to_string());
        }
        // if worst case this is equal to the data dir
        let exec_dir_index = paths.len() - 1;
        Self {
            paths,
            config_dir_index,
            data_dir_index,
            exec_dir_index,
            fs_watcher: Arc::new(Mutex::new(FileSystemWatcher::default())),
            _logger: Mutex::new(log.logger("file_system")),
            logger_fs_watch: Arc::new(Mutex::new(log.logger("file_system_watch"))),
        }
    }

    pub fn get_path(&self, path: &str, fs_path: FileSystemPath) -> ArrayString<MAX_PATH_LEN> {
        let index: usize;
        match fs_path {
            FileSystemPath::OfType(of_type) => match of_type {
                FileSystemType::ReadWrite => index = self.config_dir_index,
                FileSystemType::Read => index = self.data_dir_index,
                FileSystemType::Exec => index = self.exec_dir_index,
            },
            FileSystemPath::Index(path_index) => index = path_index,
        }
        let mut res = ArrayString::<MAX_PATH_LEN>::from(&self.paths[index].as_str()).unwrap();
        res.push_str(path);
        res
    }

    pub async fn open_file_in(
        &self,
        file_path: &str,
        path: FileSystemPath,
    ) -> std::io::Result<Vec<u8>> {
        tokio::fs::read(self.get_path(file_path, path).as_str()).await
    }

    pub async fn open_file(&self, file_path: &str) -> std::io::Result<Vec<u8>> {
        for (path_index, _) in self.paths.iter().enumerate() {
            let file = self
                .open_file_in(file_path, FileSystemPath::Index(path_index))
                .await;
            if file.is_ok() {
                return file;
            }
        }
        Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "file not found",
        ))
    }

    pub async fn write_file(&self, file_path: &str, data: Vec<u8>) -> std::io::Result<()> {
        let write_path =
            self.get_path(file_path, FileSystemPath::OfType(FileSystemType::ReadWrite));
        tokio::fs::write(write_path.to_string(), data).await
    }

    pub async fn create_dir(&self, dir_path: &str) -> std::io::Result<()> {
        let write_path = self.get_path(dir_path, FileSystemPath::OfType(FileSystemType::ReadWrite));
        tokio::fs::create_dir_all(write_path.to_string()).await
    }

    async fn files_of_dir_impl<'a, T>(
        &self,
        path: &str,
        file_read_cb: &'a mut T,
        fs_path: FileSystemPath,
        file_list: &mut Vec<String>,
    ) where
        T: FnMut(String, Vec<u8>) + Send + Sync,
    {
        let full_path = self.get_path(path, fs_path);
        let mut dir_read = tokio::fs::read_dir(full_path.as_str()).await;

        if let Ok(dir_reader) = &mut dir_read {
            while let Ok(Some(entry)) = dir_reader.next_entry().await {
                let file_type_res = entry.file_type().await;
                let file_name = entry.file_name().to_str().unwrap().to_string();
                if let Ok(file_type) = file_type_res {
                    if file_type.is_file() && !file_list.contains(&file_name) {
                        let file = tokio::fs::read(full_path.to_string() + &file_name).await;
                        if let Ok(f) = file {
                            file_list.push(file_name.clone());
                            file_read_cb(file_name, f);
                        }
                    }
                }
            }
        }
    }

    pub async fn files_of_dir<'a, T>(&self, path: &str, file_read_cb: &'a mut T)
    where
        T: FnMut(String, Vec<u8>) + Send + Sync,
    {
        let mut file_list = Vec::<String>::new();
        for (path_index, _) in self.paths.iter().enumerate() {
            self.files_of_dir_impl(
                path,
                file_read_cb,
                FileSystemPath::Index(path_index),
                &mut file_list,
            )
            .await;
        }
    }

    pub fn watch_for_change(&self, path: &str) -> FileSystemWatcherItem {
        let watch_path = self.get_path(path, FileSystemPath::OfType(FileSystemType::ReadWrite));
        FileSystemWatcherItem::new(&self.logger_fs_watch, &watch_path, &self.fs_watcher)
    }
}
