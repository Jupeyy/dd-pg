use std::{
    collections::{HashMap, HashSet},
    ffi::OsString,
    path::{Path, PathBuf},
    sync::{atomic::AtomicBool, mpsc::channel, Arc, Mutex, RwLock},
    thread::JoinHandle,
};

use async_trait::async_trait;
use base_io_traits::fs_traits::{
    FileSystemInterface, FileSystemPath, FileSystemType, FileSystemWatcherItemInterface,
};
use base_log::log::{LogLevel, SystemLog, SystemLogGroup, SystemLogInterface};
use directories::ProjectDirs;
use hashlink::LinkedHashMap;
use notify::{recommended_watcher, RecommendedWatcher, RecursiveMode, Watcher};

#[derive(Debug)]
struct FileSystemWatcherPath {
    watchers_of_path: Arc<RwLock<LinkedHashMap<usize, Arc<AtomicBool>>>>,
    watcher: Option<RecommendedWatcher>,
    thread: Option<JoinHandle<()>>,
    path: PathBuf,
    logger: Arc<Mutex<SystemLogGroup>>,
}

impl FileSystemWatcherPath {
    pub fn new(logger: &Arc<Mutex<SystemLogGroup>>, path: &Path, file: Option<&Path>) -> Self {
        // Create a channel to receive the events.
        let (tx, rx) = channel();

        // Create a watcher object, delivering debounced events.
        // The notification back-end is selected based on the platform.
        let mut watcher = recommended_watcher(tx).unwrap();

        // Add a path to be watched. All files and directories at that path and
        // below will be monitored for changes.
        if let Err(err) = watcher.watch(path, RecursiveMode::Recursive) {
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
        let file_thread = file.map(|file| {
            let mut actual_path = PathBuf::from(path);
            actual_path.push(file);
            actual_path
        });

        let watch_thread = std::thread::Builder::new()
            .name(format!("file-watcher"))
            .spawn(move || loop {
                match rx.recv() {
                    Ok(res) => match res {
                        Ok(ev) => {
                            let mut handle_ev = match ev.kind {
                                notify::EventKind::Any => false,
                                notify::EventKind::Access(_) => false,
                                notify::EventKind::Create(_) => true,
                                notify::EventKind::Modify(_) => true,
                                notify::EventKind::Remove(_) => true,
                                notify::EventKind::Other => false,
                            };
                            if let Some(file) = &file_thread {
                                // check if the file exists
                                if !ev.paths.iter().find(|path| file.eq(*path)).is_some() {
                                    handle_ev = false;
                                }
                            }
                            if handle_ev {
                                watchers_of_path_thread
                                    .read()
                                    .as_ref()
                                    .unwrap()
                                    .values()
                                    .for_each(|watcher_bool| {
                                        watcher_bool
                                            .store(true, std::sync::atomic::Ordering::Relaxed)
                                    });
                            }
                        }
                        Err(_) => {}
                    },
                    Err(_) => {
                        return;
                    }
                }
                ()
            })
            .unwrap();

        Self {
            watchers_of_path,
            watcher: Some(watcher),
            thread: Some(watch_thread),
            path: path.into(),
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

#[derive(Debug, Default)]
struct FileSystemWatcher {
    path_watchers: HashMap<PathBuf, FileSystemWatcherPath>,
    path_watcher_id_generator: usize,
}

pub struct FileSystemWatcherItem {
    fs_watcher: Arc<Mutex<FileSystemWatcher>>,
    path_watcher_id: usize,
    is_changed: Arc<AtomicBool>,
    path: PathBuf,
}

impl FileSystemWatcherItem {
    fn new(
        logger: &Arc<Mutex<SystemLogGroup>>,
        path: &Path,
        file: Option<&Path>,
        fs_watcher: &Arc<Mutex<FileSystemWatcher>>,
    ) -> Self {
        let mut actual_path = PathBuf::from(path);
        if let Some(file) = file {
            actual_path.push(file);
        }
        let mut fs_watcher_write = fs_watcher.lock().unwrap();
        let path_watcher_id = fs_watcher_write.path_watcher_id_generator;
        fs_watcher_write.path_watcher_id_generator += 1;
        let is_changed = Arc::new(AtomicBool::new(false));
        if let Some(path_watcher) = fs_watcher_write.path_watchers.get_mut(&actual_path) {
            path_watcher
                .watchers_of_path
                .write()
                .as_mut()
                .unwrap()
                .insert(path_watcher_id, is_changed.clone());
        } else {
            let path_watcher = FileSystemWatcherPath::new(logger, path, file);
            path_watcher
                .watchers_of_path
                .write()
                .as_mut()
                .unwrap()
                .insert(path_watcher_id, is_changed.clone());
            fs_watcher_write
                .path_watchers
                .insert(actual_path.clone(), path_watcher);
        }

        Self {
            fs_watcher: fs_watcher.clone(),
            path_watcher_id,
            is_changed,
            path: actual_path,
        }
    }
}

impl FileSystemWatcherItemInterface for FileSystemWatcherItem {
    fn has_file_change(&self) -> bool {
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
        let path_watcher = fs_watcher_write.path_watchers.get_mut(&self.path).unwrap();
        let mut watchers_of_path_guard = path_watcher.watchers_of_path.write();
        let watchers_of_path = watchers_of_path_guard.as_mut().unwrap();
        watchers_of_path.remove(&self.path_watcher_id);
        let watchers_empty = watchers_of_path.is_empty();
        drop(watchers_of_path_guard);
        if watchers_empty {
            fs_watcher_write.path_watchers.remove(&self.path);
        }
    }
}

#[derive(Debug)]
pub struct FileSystem {
    paths: Vec<PathBuf>,
    config_dir_index: usize,
    data_dir_index: usize,
    exec_dir_index: usize,

    fs_watcher: Arc<Mutex<FileSystemWatcher>>,
    _logger: Mutex<SystemLogGroup>,
    logger_fs_watch: Arc<Mutex<SystemLogGroup>>,
}

impl FileSystem {
    pub fn new(log: &SystemLog, qualifier: &str, organization: &str, application: &str) -> Self {
        let logger = log.logger("file_system");
        let mut config_dir: PathBuf = PathBuf::new();
        if let Some(proj_dirs) = ProjectDirs::from(qualifier, organization, application) {
            config_dir = proj_dirs.config_dir().into();
            std::fs::DirBuilder::new()
                .recursive(true)
                .create(&config_dir)
                .unwrap();
        }
        let mut paths: Vec<PathBuf> = Vec::new();
        logger
            .log(LogLevel::Info)
            .msg(&format!("Found config dir in {config_dir:?}"));
        paths.push(config_dir);
        let config_dir_index = paths.len() - 1;
        paths.push("data/".into());
        let data_dir_index = paths.len() - 1;
        if let Ok(exec_path) = std::env::current_dir() {
            paths.push(exec_path.into());
        }
        // if worst case this is equal to the data dir
        let exec_dir_index = paths.len() - 1;
        Self {
            paths,
            config_dir_index,
            data_dir_index,
            exec_dir_index,
            fs_watcher: Arc::new(Mutex::new(FileSystemWatcher::default())),
            _logger: Mutex::new(logger),
            logger_fs_watch: Arc::new(Mutex::new(log.logger("file_system_watch"))),
        }
    }

    fn get_path(&self, path: &Path, fs_path: FileSystemPath) -> PathBuf {
        let index: usize;
        match fs_path {
            FileSystemPath::OfType(of_type) => match of_type {
                FileSystemType::ReadWrite => index = self.config_dir_index,
                FileSystemType::Read => index = self.data_dir_index,
                FileSystemType::Exec => index = self.exec_dir_index,
            },
            FileSystemPath::Index(path_index) => index = path_index,
        }
        let mut res = self.paths[index].clone();
        res.push(path);
        res
    }

    async fn files_of_dir_impl(
        &self,
        path: &Path,
        file_read_cb: &mut (dyn FnMut(OsString, Vec<u8>) + Send + Sync),
        fs_path: FileSystemPath,
        file_list: &mut HashSet<OsString>,
    ) {
        let full_path = self.get_path(path, fs_path);
        let mut dir_read = tokio::fs::read_dir(&full_path).await;

        if let Ok(dir_reader) = &mut dir_read {
            while let Ok(Some(entry)) = dir_reader.next_entry().await {
                let file_type_res = entry.file_type().await;
                let file_name = entry.file_name();
                if let Ok(file_type) = file_type_res {
                    if file_type.is_file() && !file_list.contains(&file_name) {
                        let file = tokio::fs::read(full_path.join(&file_name)).await;
                        if let Ok(f) = file {
                            file_list.insert(file_name.clone());
                            file_read_cb(file_name, f);
                        }
                    }
                }
            }
        }
    }

    async fn files_in_dir_recursive_impl(
        &self,
        path: &Path,
        rec_path: PathBuf,
        fs_path: FileSystemPath,
    ) -> anyhow::Result<HashMap<PathBuf, Vec<u8>>> {
        let mut read_dirs = vec![rec_path.clone()];
        let mut file_list: HashMap<PathBuf, Vec<u8>> = Default::default();

        while let Some(rec_path) = read_dirs.pop() {
            let path = path.join(&rec_path);
            let full_path = self.get_path(&path, fs_path);
            let mut dir_reader = tokio::fs::read_dir(full_path).await?;

            while let Ok(Some(entry)) = dir_reader.next_entry().await {
                let file_type_res = entry.file_type().await;

                let entry_name = entry.file_name();
                let file_path = rec_path.join(&entry_name);
                if let Ok(file_type) = file_type_res {
                    if file_type.is_file() && !file_list.contains_key(&file_path) {
                        let file = self
                            .open_file_in(path.join(&entry_name).as_ref(), fs_path)
                            .await?;
                        file_list.insert(file_path.clone(), file);
                    } else if file_type.is_dir() {
                        read_dirs.push(file_path.into());
                    }
                }
            }
        }

        Ok(file_list)
    }
}

#[async_trait]
impl FileSystemInterface for FileSystem {
    async fn open_file(&self, file_path: &Path) -> std::io::Result<Vec<u8>> {
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
            format!("file not found: {file_path:?}"),
        ))
    }

    async fn open_file_in(
        &self,
        file_path: &Path,
        path: FileSystemPath,
    ) -> std::io::Result<Vec<u8>> {
        tokio::fs::read(self.get_path(file_path, path)).await
    }

    async fn file_exists(&self, file_path: &Path) -> bool {
        let path = self.get_path(file_path, FileSystemPath::OfType(FileSystemType::ReadWrite));
        tokio::fs::try_exists(path).await.unwrap_or_default()
    }

    async fn write_file(&self, file_path: &Path, data: Vec<u8>) -> std::io::Result<()> {
        let write_path =
            self.get_path(file_path, FileSystemPath::OfType(FileSystemType::ReadWrite));

        tokio::fs::write(write_path, data).await
    }

    async fn create_dir(&self, dir_path: &Path) -> std::io::Result<()> {
        let write_path = self.get_path(dir_path, FileSystemPath::OfType(FileSystemType::ReadWrite));
        tokio::fs::create_dir_all(write_path).await
    }

    async fn files_of_dir(
        &self,
        path: &Path,
        file_read_cb: &mut (dyn FnMut(OsString, Vec<u8>) + Send + Sync),
    ) {
        let mut file_list = HashSet::<OsString>::new();
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

    async fn files_in_dir_recursive(
        &self,
        path: &Path,
    ) -> anyhow::Result<HashMap<PathBuf, Vec<u8>>> {
        let mut file_list = HashMap::<PathBuf, Vec<u8>>::new();
        let mut found_one_dir = false;
        for (path_index, _) in self.paths.iter().enumerate() {
            if let Ok(list) = self
                .files_in_dir_recursive_impl(path, "".into(), FileSystemPath::Index(path_index))
                .await
            {
                found_one_dir = true;
                file_list.extend(list.into_iter());
            }
        }

        if found_one_dir {
            Ok(file_list)
        } else {
            Err(anyhow::anyhow!("no directory with in {:?} was found", path))
        }
    }

    fn get_save_path(&self) -> PathBuf {
        self.get_path(
            ".".as_ref(),
            FileSystemPath::OfType(FileSystemType::ReadWrite),
        )
    }

    fn watch_for_change(
        &self,
        path: &Path,
        file: Option<&Path>,
    ) -> Box<dyn FileSystemWatcherItemInterface> {
        let watch_path = self.get_path(path, FileSystemPath::OfType(FileSystemType::ReadWrite));
        Box::new(FileSystemWatcherItem::new(
            &self.logger_fs_watch,
            &watch_path,
            file,
            &self.fs_watcher,
        ))
    }
}

#[cfg(test)]
mod test {
    use base_io_traits::fs_traits::FileSystemInterface;
    use base_log::log::SystemLog;

    use super::FileSystem;

    #[tokio::test]
    async fn read_dir_recursive() {
        let workspace_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../");
        std::env::set_current_dir(workspace_root).unwrap();
        let fs = FileSystem::new(&SystemLog::new(), "ddnet-test", "ddnet-test", "ddnet-test");

        let files = fs
            .files_in_dir_recursive("data/skins/default".as_ref())
            .await
            .unwrap();
        assert!(files.len() > 0);
        for (path, _) in files {
            println!("{:?}", path.to_string_lossy());
        }
    }
}
