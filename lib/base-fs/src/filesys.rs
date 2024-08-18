use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::{atomic::AtomicBool, mpsc::channel, Arc, Mutex, RwLock},
    thread::JoinHandle,
    time::Duration,
};

use async_trait::async_trait;
use base_io_traits::fs_traits::{
    FileSystemInterface, FileSystemPath, FileSystemType, FileSystemWatcherItemInterface,
};
use directories::ProjectDirs;
use hashlink::LinkedHashMap;
use notify::{recommended_watcher, RecommendedWatcher, RecursiveMode, Watcher};
use path_slash::PathBufExt;
use virtual_fs::{
    mem_fs, AsyncReadExt, AsyncWriteExt, OpenOptionsConfig, ScopedDirectoryFileSystem,
};

#[derive(Debug)]
struct FileSystemWatcherPath {
    watchers_of_path: Arc<RwLock<LinkedHashMap<usize, Arc<AtomicBool>>>>,
    watcher: Option<RecommendedWatcher>,
    thread: Option<JoinHandle<()>>,
    path: PathBuf,
}

impl FileSystemWatcherPath {
    pub fn new(path: &Path, file: Option<&Path>) -> Self {
        // Create a channel to receive the events.
        let (tx, rx) = channel();

        // Create a watcher object, delivering debounced events.
        // The notification back-end is selected based on the platform.
        let mut watcher = recommended_watcher(tx).unwrap();

        // Add a path to be watched. All files and directories at that path and
        // below will be monitored for changes.
        if let Err(err) = watcher.watch(path, RecursiveMode::Recursive) {
            log::info!(target: "fs-watch", "could not watch directory/file: {err:}");
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
            .name("file-watcher".to_string())
            .spawn(move || loop {
                match rx.recv() {
                    Ok(res) => {
                        if let Ok(ev) = res {
                            let mut handle_ev = match ev.kind {
                                notify::EventKind::Any => false,
                                notify::EventKind::Other => false,
                                notify::EventKind::Access(ev) => match ev {
                                    notify::event::AccessKind::Any => false,
                                    notify::event::AccessKind::Read => false,
                                    notify::event::AccessKind::Open(_) => false,
                                    notify::event::AccessKind::Close(ev) => match ev {
                                        notify::event::AccessMode::Any => false,
                                        notify::event::AccessMode::Execute => false,
                                        notify::event::AccessMode::Read => false,
                                        notify::event::AccessMode::Write => true,
                                        notify::event::AccessMode::Other => false,
                                    },
                                    notify::event::AccessKind::Other => false,
                                },
                                notify::EventKind::Create(ev) => match ev {
                                    notify::event::CreateKind::Any => false,
                                    notify::event::CreateKind::Other => false,
                                    notify::event::CreateKind::File => true,
                                    notify::event::CreateKind::Folder => true,
                                },
                                notify::EventKind::Modify(_) => {
                                    // only listen for modify events in a pure directory mode
                                    file_thread.is_none()
                                }
                                notify::EventKind::Remove(ev) => match ev {
                                    notify::event::RemoveKind::Any => false,
                                    notify::event::RemoveKind::Other => false,
                                    notify::event::RemoveKind::File => true,
                                    notify::event::RemoveKind::Folder => true,
                                },
                            };
                            if let Some(file) = &file_thread {
                                // check if the file exists
                                if !ev.paths.iter().any(|path| file.eq(path)) {
                                    handle_ev = false;
                                }
                            }
                            if handle_ev {
                                // if the file exist, make sure the file is not modified for at least 1 second
                                if let Some(file_thread) = &file_thread {
                                    let mut last_modified = None;

                                    while let Ok(file) = std::fs::File::open(file_thread) {
                                        if let Some(modified) = file
                                            .metadata()
                                            .ok()
                                            .and_then(|metadata| metadata.modified().ok())
                                        {
                                            if let Some(file_last_modified) = last_modified {
                                                if modified == file_last_modified {
                                                    break;
                                                } else {
                                                    // else try again
                                                    last_modified = Some(modified);
                                                }
                                            } else {
                                                last_modified = Some(modified);
                                            }
                                        } else {
                                            break;
                                        }
                                        drop(file);
                                        std::thread::sleep(Duration::from_secs(1));
                                    }
                                }

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
                    }
                    Err(_) => {
                        return;
                    }
                }
            })
            .unwrap();

        Self {
            watchers_of_path,
            watcher: Some(watcher),
            thread: Some(watch_thread),
            path: path.into(),
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
            log::info!(target: "fs-watch", "could not stop watching directory/file: {err}");
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
    fn new(path: &Path, file: Option<&Path>, fs_watcher: &Arc<Mutex<FileSystemWatcher>>) -> Self {
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
            let path_watcher = FileSystemWatcherPath::new(path, file);
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

pub trait ScopedDirFileSystemInterface: virtual_fs::FileSystem + virtual_fs::FileOpener {}

impl ScopedDirFileSystemInterface for ScopedDirectoryFileSystem {}
impl ScopedDirFileSystemInterface for mem_fs::FileSystem {}

#[derive(Debug)]
pub struct ScopedDirFileSystem {
    fs: Box<dyn ScopedDirFileSystemInterface>,
    host_path: PathBuf,
    mount_path: PathBuf,
}

impl ScopedDirFileSystem {
    pub fn get_path(&self, path: impl AsRef<Path>) -> PathBuf {
        path_clean::clean(self.mount_path.join(path.as_ref()))
    }
}

#[derive(Debug)]
pub struct FileSystem {
    scoped_file_systems: Vec<ScopedDirFileSystem>,
    config_dir_index: usize,
    data_dir_index: usize,
    exec_dir_index: usize,

    fs_watcher: Arc<Mutex<FileSystemWatcher>>,

    secure_path: PathBuf,
}

impl FileSystem {
    #[cfg(not(feature = "bundled_data_dir"))]
    fn add_data_dir(scoped_file_systems: &mut Vec<ScopedDirFileSystem>) -> usize {
        scoped_file_systems.push(ScopedDirFileSystem {
            fs: Box::new(ScopedDirectoryFileSystem::new_with_default_runtime("data/")),
            host_path: "data/".into(),
            mount_path: "".into(),
        });
        scoped_file_systems.len() - 1
    }
    #[cfg(feature = "bundled_data_dir")]
    fn add_data_dir(scoped_file_systems: &mut Vec<ScopedDirFileSystem>) -> usize {
        const DATA_DIR: include_dir::Dir =
            include_dir::include_dir!("$CARGO_MANIFEST_DIR/../../data");

        let fs: Box<dyn ScopedDirFileSystemInterface> = Box::new(mem_fs::FileSystem::default());

        fn add_dirs(fs: &dyn ScopedDirFileSystemInterface, dir: &include_dir::Dir) {
            let add_file = |dir: &include_dir::Dir| {
                for file in dir.files() {
                    let mut fs_file = fs
                        .open(
                            &PathBuf::from("/").join(file.path()),
                            &OpenOptionsConfig {
                                read: false,
                                write: true,
                                create_new: true,
                                create: true,
                                append: false,
                                truncate: false,
                            },
                        )
                        .unwrap();
                    tokio::runtime::Handle::current()
                        .block_on(fs_file.write_all(file.contents()))
                        .unwrap();
                }
            };
            for dir in dir.dirs() {
                fs.create_dir(PathBuf::from("/").join(dir.path()).as_path())
                    .unwrap();

                add_file(dir);
                add_dirs(fs, dir);
            }
        }

        add_dirs(fs.as_ref(), &DATA_DIR);

        scoped_file_systems.push(ScopedDirFileSystem {
            fs,
            host_path: "data/".into(),
            mount_path: "/".into(),
        });
        scoped_file_systems.len() - 1
    }

    pub fn new(
        rt: &tokio::runtime::Runtime,
        qualifier: &str,
        organization: &str,
        application: &str,
        secure_appl: &str,
    ) -> Self {
        let mut config_dir: PathBuf = PathBuf::new();
        if let Some(proj_dirs) = ProjectDirs::from(qualifier, organization, application) {
            config_dir = proj_dirs.config_dir().into();
            std::fs::DirBuilder::new()
                .recursive(true)
                .create(&config_dir)
                .unwrap();
        }
        let mut secure_dir: PathBuf = PathBuf::new();
        if let Some(proj_dirs) = ProjectDirs::from(qualifier, organization, secure_appl) {
            secure_dir = proj_dirs.data_dir().into();
            std::fs::DirBuilder::new()
                .recursive(true)
                .create(&secure_dir)
                .unwrap();
        }
        // enter tokio runtime for [ScopedDirectoryFileSystem::new_with_default_runtime]
        let g = rt.enter();
        let mut scoped_file_systems: Vec<ScopedDirFileSystem> = Vec::new();
        log::info!(target: "fs", "Found config dir in {config_dir:?}");
        scoped_file_systems.push(ScopedDirFileSystem {
            fs: Box::new(ScopedDirectoryFileSystem::new_with_default_runtime(
                config_dir.clone(),
            )),
            host_path: config_dir,
            mount_path: "".into(),
        });
        let config_dir_index = scoped_file_systems.len() - 1;
        let data_dir_index = Self::add_data_dir(&mut scoped_file_systems);
        if let Ok(exec_path) = std::env::current_dir() {
            scoped_file_systems.push(ScopedDirFileSystem {
                fs: Box::new(ScopedDirectoryFileSystem::new_with_default_runtime(
                    exec_path.clone(),
                )),
                host_path: exec_path,
                mount_path: "".into(),
            });
        }
        drop(g);
        // if worst case this is equal to the data dir
        let exec_dir_index = scoped_file_systems.len() - 1;
        Self {
            scoped_file_systems,
            config_dir_index,
            data_dir_index,
            exec_dir_index,
            fs_watcher: Arc::new(Mutex::new(FileSystemWatcher::default())),

            secure_path: secure_dir,
        }
    }

    fn get_scoped_fs(&self, fs_path: FileSystemPath) -> &ScopedDirFileSystem {
        let index: usize;
        match fs_path {
            FileSystemPath::OfType(of_type) => match of_type {
                FileSystemType::ReadWrite => index = self.config_dir_index,
                FileSystemType::Read => index = self.data_dir_index,
                FileSystemType::Exec => index = self.exec_dir_index,
            },
            FileSystemPath::Index(path_index) => index = path_index,
        }
        &self.scoped_file_systems[index]
    }

    fn get_path(&self, path: &Path, fs_path: FileSystemPath) -> PathBuf {
        let mut res = self.get_scoped_fs(fs_path).host_path.clone();
        res.push(path);
        res
    }

    async fn entries_in_dir_impl(
        &self,
        path: &Path,
        fs: &ScopedDirFileSystem,
    ) -> anyhow::Result<HashSet<String>> {
        let mut file_list: HashSet<String> = Default::default();
        let mut dir_read = virtual_fs::FileSystem::read_dir(&fs.fs, path)?;

        while let Some(Ok(entry)) = dir_read.next() {
            let file_name = entry.file_name();
            file_list.insert(file_name.to_string_lossy().to_string());
        }
        Ok(file_list)
    }

    async fn files_in_dir_recursive_impl(
        &self,
        path: &Path,
        rec_path: PathBuf,
        fs: &ScopedDirFileSystem,
        fs_path: FileSystemPath,
    ) -> anyhow::Result<HashMap<PathBuf, Vec<u8>>> {
        let mut read_dirs = vec![rec_path.clone()];
        let mut file_list: HashMap<PathBuf, Vec<u8>> = Default::default();

        while let Some(rec_path) = read_dirs.pop() {
            let path = path.join(&rec_path);
            let mut dir_reader = virtual_fs::FileSystem::read_dir(&fs.fs, &path)?;

            while let Some(Ok(entry)) = dir_reader.next() {
                let file_type_res = entry.file_type();

                let entry_name = entry.file_name();
                let file_path = rec_path.join(&entry_name);
                if let Ok(file_type) = file_type_res {
                    if file_type.is_file() && !file_list.contains_key(&file_path) {
                        let file = self
                            .read_file_in(path.join(&entry_name).as_ref(), fs_path)
                            .await?;
                        let file_path_slash = file_path.to_slash_lossy().as_ref().into();
                        file_list.insert(file_path_slash, file);
                    } else if file_type.is_dir() {
                        read_dirs.push(file_path);
                    }
                }
            }
        }

        Ok(file_list)
    }
}

#[async_trait]
impl FileSystemInterface for FileSystem {
    async fn read_file(&self, file_path: &Path) -> std::io::Result<Vec<u8>> {
        for fs in self.scoped_file_systems.iter() {
            if let Ok(mut file) = {
                let file_path = fs.get_path(file_path);
                fs.fs.open(
                    &file_path,
                    &OpenOptionsConfig {
                        read: true,
                        write: false,
                        create_new: false,
                        create: false,
                        append: false,
                        truncate: false,
                    },
                )
            } {
                let mut file_res: Vec<_> = Default::default();
                if file.read_to_end(&mut file_res).await.is_ok() {
                    return Ok(file_res);
                }
            }
        }
        Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("file not found: {file_path:?}"),
        ))
    }

    async fn read_file_in(
        &self,
        file_path: &Path,
        path: FileSystemPath,
    ) -> std::io::Result<Vec<u8>> {
        let fs = self.get_scoped_fs(path);
        let file_path = fs.get_path(file_path);
        let mut file = fs.fs.open(
            &file_path,
            &OpenOptionsConfig {
                read: true,
                write: false,
                create_new: false,
                create: false,
                append: false,
                truncate: false,
            },
        )?;
        let mut file_res: Vec<_> = Default::default();
        file.read_to_end(&mut file_res).await?;
        Ok(file_res)
    }

    async fn file_exists(&self, file_path: &Path) -> bool {
        let fs = self.get_scoped_fs(FileSystemPath::OfType(FileSystemType::ReadWrite));
        let file_path = fs.get_path(file_path);
        fs.fs
            .open(
                &file_path,
                &OpenOptionsConfig {
                    read: true,
                    write: false,
                    create_new: false,
                    create: false,
                    append: false,
                    truncate: false,
                },
            )
            .is_ok()
    }

    async fn write_file(&self, file_path: &Path, data: Vec<u8>) -> std::io::Result<()> {
        let fs = self.get_scoped_fs(FileSystemPath::OfType(FileSystemType::ReadWrite));

        let options = OpenOptionsConfig {
            read: false,
            write: true,
            create_new: true,
            create: true,
            append: false,
            truncate: false,
        };
        let file_path = fs.get_path(file_path);

        let file_res = fs.fs.open(&file_path, &options);
        let mut file = match file_res {
            Ok(file) => file,
            Err(err) => match err {
                virtual_fs::FsError::AlreadyExists => {
                    fs.fs.remove_file(&file_path)?;
                    fs.fs.open(&file_path, &options)?
                }
                err => {
                    return Err(err.into());
                }
            },
        };

        file.write_all(&data).await?;
        Ok(())
    }

    async fn create_dir(&self, dir_path: &Path) -> std::io::Result<()> {
        let fs = self.get_scoped_fs(FileSystemPath::OfType(FileSystemType::ReadWrite));
        let mut cur_dir = fs.mount_path.clone();
        let dir_path = path_clean::clean(dir_path);
        let components = dir_path.components();
        for comp in components {
            cur_dir.push(comp);
            if let Err(err) = virtual_fs::FileSystem::create_dir(&fs.fs, &cur_dir) {
                match err {
                    virtual_fs::FsError::AlreadyExists => {
                        // ignore
                    }
                    err => {
                        return Err(err.into());
                    }
                }
            }
        }
        Ok(())
    }

    async fn entries_in_dir(&self, path: &Path) -> anyhow::Result<HashSet<String>> {
        let mut file_list = HashSet::<String>::new();
        let mut found_one_entry = false;
        for fs in self.scoped_file_systems.iter() {
            let path = fs.get_path(path);
            if let Ok(ext_file_list) = self.entries_in_dir_impl(&path, fs).await {
                found_one_entry = true;
                file_list.extend(ext_file_list.into_iter());
            }
        }
        if found_one_entry {
            Ok(file_list)
        } else {
            Err(anyhow::anyhow!("no entry within {:?} was found", path))
        }
    }

    async fn files_in_dir_recursive(
        &self,
        path: &Path,
    ) -> anyhow::Result<HashMap<PathBuf, Vec<u8>>> {
        let mut file_list = HashMap::<PathBuf, Vec<u8>>::new();
        let mut found_one_dir = false;
        for (path_index, fs) in self.scoped_file_systems.iter().enumerate() {
            let path = fs.get_path(path);
            if let Ok(list) = self
                .files_in_dir_recursive_impl(
                    &path,
                    "".into(),
                    fs,
                    FileSystemPath::Index(path_index),
                )
                .await
            {
                found_one_dir = true;
                file_list.extend(list.into_iter());
            }
        }

        if found_one_dir {
            Ok(file_list)
        } else {
            Err(anyhow::anyhow!("no directory within {:?} was found", path))
        }
    }

    fn get_save_path(&self) -> PathBuf {
        self.get_path(
            ".".as_ref(),
            FileSystemPath::OfType(FileSystemType::ReadWrite),
        )
    }

    fn get_secure_path(&self) -> PathBuf {
        self.secure_path.clone()
    }

    fn watch_for_change(
        &self,
        path: &Path,
        file: Option<&Path>,
    ) -> Box<dyn FileSystemWatcherItemInterface> {
        let watch_path = self.get_path(path, FileSystemPath::OfType(FileSystemType::ReadWrite));
        Box::new(FileSystemWatcherItem::new(
            &watch_path,
            file,
            &self.fs_watcher,
        ))
    }
}
