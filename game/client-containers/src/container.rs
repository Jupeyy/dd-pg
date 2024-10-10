use rustc_hash::{FxHashMap as HashMap, FxHashSet as HashSet};
use std::{
    borrow::Borrow,
    io::Read,
    marker::PhantomData,
    path::{Path, PathBuf},
    rc::Rc,
    sync::Arc,
    time::Duration,
};

use anyhow::anyhow;
use base::hash::{fmt_hash, Hash};
use base_io_traits::{fs_traits::FileSystemInterface, http_traits::HttpClientInterface};

use base_io::{io::Io, io_batcher::IoBatcherTask};
use either::Either;
use game_interface::types::resource_key::ResourceKey;
use graphics::{
    graphics::graphics::Graphics, graphics_mt::GraphicsMultiThreaded,
    handles::texture::texture::GraphicsTextureHandle, image::texture_2d_to_3d,
};
use graphics_types::{
    commands::TexFlags,
    types::{GraphicsBackendMemory, GraphicsMemoryAllocationType},
};
use hashlink::LinkedHashMap;
use hiarc::Hiarc;
use image::png::{load_png_image, PngResultPersistent};
use log::info;
use serde::{Deserialize, Serialize};
use sound::{
    scene_object::SceneObject, sound::SoundManager, sound_handle::SoundObjectHandle,
    sound_mt::SoundMultiThreaded, sound_mt_types::SoundBackendMemory,
};
use url::Url;

#[derive(Debug, Hiarc)]
pub struct ContainerItemLoadData {
    pub width: u32,
    pub height: u32,
    pub depth: u32,
    pub data: GraphicsBackendMemory,
}

#[derive(Debug, Hiarc, Clone)]
pub struct ContainerLoadedItemDir {
    /// key is the relative path
    /// excluding the item's directory
    /// (e.g. `greyfox/*` => `*`)
    pub files: HashMap<PathBuf, Vec<u8>>,

    _dont_construct: PhantomData<()>,
}

impl ContainerLoadedItemDir {
    pub fn new(files: HashMap<PathBuf, Vec<u8>>) -> Self {
        Self {
            files,
            _dont_construct: Default::default(),
        }
    }
}

#[derive(Debug, Hiarc, Clone)]
pub enum ContainerLoadedItem {
    SingleFile(Vec<u8>),
    Directory(ContainerLoadedItemDir),
}

#[derive(Debug, Hiarc)]
struct ContainerItem<A> {
    item: A,
    used_last_in: Duration,
}

pub type ContainerKey = ResourceKey;

/// An entry on a http server
#[derive(Debug, Hiarc, Clone, Serialize, Deserialize)]
pub struct ContainerItemIndexEntry {
    pub ty: String,
    pub hash: base::hash::Hash,
}

pub type ContainerItemIndexEntries = HashMap<String, ContainerItemIndexEntry>;

/// This a hint is to determine if the container item
/// is likely be loaded by disk or http etc.
#[derive(Debug, Clone, Copy)]
pub enum ContainerItemIndexType {
    Disk,
    Http,
}

type ResourceIndex = HashSet<String>;
type TokioArcMutex<T> = Arc<tokio::sync::Mutex<T>>;

/// Containers are a collection of named assets, e.g. all skins
/// are part of the skins container.
///
/// Assets have a name and corresponding to this name
/// there are textures, sounds, effects or whatever fits the container logically.
/// All containers should have a `default` value/texture/sound etc.
///
/// # Users
/// Users of the containers must call [Container::get_or_default] to get
/// access to a resource. It accepts a file name and a optional hash.
/// The hash must be used if the resource is forced by a game server,
/// else it's optional.
/// Calling [Container::update] causes the container to remove unused
/// resources, to make sure that resources are not unloaded to often
/// you should usually pass the `force_used_items` argument which should
/// be filled with items that are likely used (e.g. from a player list).
///
/// # Implementors
/// Generally items of containers have three load modes and 2 file modes:
/// Load modes:
/// - Http server, used across all game servers + in the UI. Uses a JSON to list all entries.
/// - Game server, used for a specific game server. Must use file hashes.
/// - Local files, reading files without any hash.
///   Both http server & game server mode try to load from a disk cache first.
///   File modes:
/// - Single file: A single file, most commonly a texture, was loaded and the implementations
///     must ensure to load proper default values for other resources of an item (sounds etc.)
/// - Directory: A directory with many different resources was loaded. Missing resources must be filled
///     with values of the default item. A directory might be archieved in a .tar ball, which is automatically
///     unpacked and processed.
#[derive(Debug, Hiarc)]
pub struct Container<A, L> {
    items: LinkedHashMap<ContainerKey, ContainerItem<A>>,
    loading_tasks: HashMap<ContainerKey, Option<IoBatcherTask<L>>>,

    // containers allow to delay loading the default item as much as possible, to improve startup time
    default_item: Option<IoBatcherTask<(L, ContainerLoadedItemDir)>>,
    default_loaded_item: Arc<ContainerLoadedItemDir>,
    pub default_key: Rc<ContainerKey>,

    // strict private data
    io: Io,
    graphics_mt: GraphicsMultiThreaded,
    texture_handle: GraphicsTextureHandle,
    sound_mt: SoundMultiThreaded,
    sound_object_handle: SoundObjectHandle,
    runtime_thread_pool: Arc<rayon::ThreadPool>,
    container_name: String,

    /// url for generaly resource downloads
    resource_http_download_url: Option<Url>,
    /// url for resource downloads from a game server
    resource_server_download_url: Option<Url>,
    /// Base path to container items.
    /// This is used for disk aswell as for the http requests.
    /// (So a http server mirrors a local data directory)
    base_path: PathBuf,

    /// An index, downloaded as JSON, that contains file paths + hashes
    /// over all downloadable files of the http download
    /// server. This is downloaded once and must exist
    /// in order to download further assets from the server
    resource_http_download_index:
        Arc<tokio::sync::Mutex<Option<anyhow::Result<ContainerItemIndexEntries>>>>,

    /// A list of entries the client can load without hashes
    /// usually it makes sense to combine it with `resource_http_download_index`
    /// to get a list of loadable items
    resource_dir_index: Either<
        anyhow::Result<HashSet<String>>,
        Option<IoBatcherTask<anyhow::Result<ResourceIndex>>>,
    >,

    /// last time the container was updated by [Self::update]
    last_update_time: Option<Duration>,
    last_update_interval_time: Option<Duration>,
}

pub trait ContainerLoad<A>
where
    Self: Sized,
{
    fn load(
        item_name: &str,
        files: ContainerLoadedItem,
        default_files: &ContainerLoadedItemDir,
        runtime_thread_pool: &Arc<rayon::ThreadPool>,
        graphics_mt: &GraphicsMultiThreaded,
        sound_mt: &SoundMultiThreaded,
    ) -> anyhow::Result<Self>;

    fn convert(
        self,
        texture_handle: &GraphicsTextureHandle,
        sound_object_handle: &SoundObjectHandle,
    ) -> A;
}

impl<A, L> Container<A, L>
where
    L: ContainerLoad<A> + Sync + Send + 'static,
{
    /// Creates a new container instance.
    /// Interesting parameters are:
    /// - `resource_http_download_url`:
    ///     The resource server for general purpose, cross server resources
    /// - `resource_server_download_url`:
    ///     The resource for a game server, which are only downloaded if a hash
    ///     is provided.
    /// - `sound_scene`:
    ///     The scene in which the sounds are created in.
    pub fn new(
        io: Io,
        runtime_thread_pool: Arc<rayon::ThreadPool>,
        default_item: IoBatcherTask<ContainerLoadedItem>,
        resource_http_download_url: Option<Url>,
        resource_server_download_url: Option<Url>,
        container_name: &str,
        graphics: &Graphics,
        sound: &SoundManager,
        sound_scene: &SceneObject,
        base_path: &Path,
    ) -> Self {
        let items = LinkedHashMap::new();
        Self {
            items,
            loading_tasks: HashMap::default(),

            default_item: Some({
                let runtime_thread_pool = runtime_thread_pool.clone();
                let graphics_mt = graphics.get_graphics_mt();
                let sound_mt = sound.get_sound_mt();
                io.io_batcher.then(default_item, |default_item| async move {
                    let ContainerLoadedItem::Directory(default_item) = default_item else {
                        return Err(anyhow::anyhow!("default item must be a directory"));
                    };

                    L::load(
                        "default",
                        ContainerLoadedItem::Directory(default_item.clone()),
                        // dummy
                        &ContainerLoadedItemDir::new(Default::default()),
                        &runtime_thread_pool,
                        &graphics_mt,
                        &sound_mt,
                    )
                    .map(|item| (item, default_item))
                })
            }),
            // create a dummy, all paths must have checked if default item was loaded
            default_loaded_item: Arc::new(ContainerLoadedItemDir::new(Default::default())),
            default_key: Rc::new("default".try_into().unwrap()),

            io,
            graphics_mt: graphics.get_graphics_mt(),
            texture_handle: graphics.texture_handle.clone(),
            sound_mt: sound.get_sound_mt(),
            sound_object_handle: sound_scene.sound_object_handle.clone(),
            runtime_thread_pool,

            container_name: container_name.to_string(),

            resource_http_download_url,
            resource_server_download_url,
            base_path: base_path.to_path_buf(),

            resource_http_download_index: Default::default(),
            resource_dir_index: Either::Right(None),

            last_update_time: None,
            last_update_interval_time: None,
        }
    }

    fn check_default_loaded(&mut self) {
        // make sure default is loaded
        if let Some(default_item) = self.default_item.take() {
            let (default_item, default_loaded_item) = default_item
                .get_storage()
                .map_err(|err| {
                    anyhow!(
                        "failed to load default files for \"{}\": {err}",
                        self.base_path.to_string_lossy()
                    )
                })
                .unwrap();
            self.default_loaded_item = Arc::new(default_loaded_item);
            self.items.insert(
                (*self.default_key).clone(),
                ContainerItem {
                    item: default_item.convert(&self.texture_handle, &self.sound_object_handle),
                    used_last_in: Duration::ZERO,
                },
            );
        }
    }

    /// Update this container, removing unused items
    ///
    /// `update_interval` is the time to wait before doing another update check.
    /// This allows to save some runtime cost.
    pub fn update<'a>(
        &mut self,
        cur_time: &Duration,
        entry_lifetime: &Duration,
        update_interval: &Duration,
        force_used_items: impl Iterator<Item = &'a ContainerKey>,
    ) {
        if self
            .last_update_interval_time
            .is_none_or(|time| cur_time.saturating_sub(time) >= *update_interval)
        {
            self.last_update_interval_time = Some(*cur_time);

            self.check_default_loaded();

            // make sure these entries are always kept loaded
            for force_used_item in force_used_items {
                if let Some(item) = self.items.to_back(force_used_item) {
                    item.used_last_in = *cur_time;
                }
            }

            // all items that were not used lately
            // are always among the first items
            // delete them if they were not used lately
            while !self.items.is_empty() {
                let (name, item) = self.items.iter_mut().next().unwrap();
                if self.last_update_time.is_some_and(|last_update_time| {
                    last_update_time.saturating_sub(item.used_last_in) > *entry_lifetime
                }) && !name.eq(&self.default_key)
                {
                    let name_clone = name.clone();
                    let _ = self.items.remove(&name_clone).unwrap();
                } else {
                    break;
                }
            }
            let item = self.items.to_back(&self.default_key).unwrap();
            item.used_last_in = *cur_time;
        }
        self.last_update_time = Some(*cur_time);
    }

    async fn load_container_item(
        container_name: String,
        fs: Arc<dyn FileSystemInterface>,
        http: Arc<dyn HttpClientInterface>,
        base_path: PathBuf,
        key: ContainerKey,
        game_server_http: Option<Url>,
        resource_http_download: Option<(
            TokioArcMutex<Option<anyhow::Result<ContainerItemIndexEntries>>>,
            Url,
        )>,
    ) -> anyhow::Result<ContainerLoadedItem> {
        let read_tar = |file: &[u8]| {
            let mut file = tar::Archive::new(std::io::Cursor::new(file));
            match file.entries() {
                Ok(entries) => entries
                    .map(|entry| {
                        entry
                            .map(|mut entry| {
                                let path = entry.path().map(|path| path.to_path_buf())?;
                                let mut file: Vec<_> = Default::default();
                                entry.read_to_end(&mut file).map(|_| (path, file))
                            })
                            .map_err(|err| anyhow::anyhow!(err))
                            .and_then(|val| anyhow::Ok(val?))
                    })
                    .collect::<anyhow::Result<HashMap<_, _>>>(),
                Err(err) => Err(anyhow::anyhow!(err)),
            }
        };

        // if key hash a hash, try to load item with that hash from disk
        // or download it from the game server if supported
        // else it will be ignored
        let files = if let Some(hash) = key.hash {
            // try to load dir with that name
            let mut files = None;

            if let Ok(dir_files) = fs
                .files_in_dir_recursive(&base_path.join(format!(
                    "{}_{}",
                    key.name.as_str(),
                    fmt_hash(&hash)
                )))
                .await
            {
                files = Some(ContainerLoadedItem::Directory(ContainerLoadedItemDir::new(
                    dir_files,
                )));
            }

            // else try to load tar with that name
            if files.is_none() {
                if let Ok(file) = fs
                    .read_file(&base_path.join(format!(
                        "{}_{}.tar",
                        key.name.as_str(),
                        fmt_hash(&hash)
                    )))
                    .await
                {
                    if let Ok(tar_files) = read_tar(&file) {
                        files = Some(ContainerLoadedItem::Directory(ContainerLoadedItemDir::new(
                            tar_files,
                        )));
                    }
                }
            }

            // else try to load single file (.png, .ogg or similar)
            // Note: for now only try image files, doesn't seem worth it for sound files
            if files.is_none() {
                if let Ok(file) = fs
                    .read_file(&base_path.join(format!(
                        "{}_{}.png",
                        key.name.as_str(),
                        fmt_hash(&hash)
                    )))
                    .await
                {
                    files = Some(ContainerLoadedItem::SingleFile(file));
                }
            }

            // if loading still failed, switch to http download
            if files.is_none() {
                if let Some(game_server_http) = game_server_http.as_ref().and_then(|url| {
                    url.join(&format!("{}_{}.tar", key.name.as_str(), fmt_hash(&hash)))
                        .ok()
                }) {
                    if let Ok(file) = http.download_binary(game_server_http, &hash).await {
                        if let Ok(tar_files) = read_tar(&file) {
                            files = Some(ContainerLoadedItem::Directory(
                                ContainerLoadedItemDir::new(tar_files),
                            ));
                        }
                    }
                }
            }

            // at last, try a single .png, .ogg file etc.
            // Note: for now only try image files, doesn't seem worth it for sound files
            if files.is_none() {
                if let Some(game_server_http) = game_server_http.as_ref().and_then(|url| {
                    url.join(&format!("{}_{}.png", key.name.as_str(), fmt_hash(&hash)))
                        .ok()
                }) {
                    if let Ok(file) = http.download_binary(game_server_http, &hash).await {
                        files = Some(ContainerLoadedItem::SingleFile(file.to_vec()));
                    }
                }
            }

            match files {
                Some(files) => Ok(files),
                None => Err(anyhow!(
                    "Could not load/download resource with name {} and hash {}",
                    key.name.as_str(),
                    fmt_hash(&hash)
                )),
            }
        } else {
            let http_entry = if let Some((http_index, resource_http_download_url)) =
                resource_http_download
            {
                let mut http_index = http_index.lock().await;

                // try to download index
                if http_index.is_none() {
                    if let Some(base_path) = base_path.to_str().and_then(|base_path| {
                        resource_http_download_url
                            .join(base_path)
                            .and_then(|path| path.join("index.json"))
                            .ok()
                    }) {
                        let r = http
                            .download_text(base_path)
                            .await
                            .map_err(|err| anyhow!(err))
                            .and_then(|index_file| {
                                serde_json::from_str::<ContainerItemIndexEntries>(&index_file)
                                    .map_err(|err| anyhow::anyhow!(err))
                            });

                        if let Err(err) = &r {
                            info!(target: &container_name, "failed to create http index for {container_name}: {err}");
                        }

                        *http_index = Some(r);
                    }
                }

                http_index
                    .as_mut()
                    .and_then(|entries| {
                        entries
                            .as_ref()
                            .ok()
                            .map(|entries| entries.get(key.name.as_str()).cloned())
                    })
                    .flatten()
                    .map(|entry| (entry, resource_http_download_url))
            } else {
                None
            };

            let mut files = None;
            // if an entry exists, first try to load from disk using the entries hash
            if let Some((entry, _)) = &http_entry {
                if let Ok(file) = fs
                    .read_file(
                        format!(
                            "{}_{}.{}",
                            key.name.as_str(),
                            fmt_hash(&entry.hash),
                            entry.ty
                        )
                        .as_ref(),
                    )
                    .await
                {
                    if entry.ty == "tar" {
                        if let Ok(tar_files) = read_tar(&file) {
                            files = Some(ContainerLoadedItem::Directory(
                                ContainerLoadedItemDir::new(tar_files),
                            ));
                        }
                    } else if entry.ty == "png" {
                        files = Some(ContainerLoadedItem::SingleFile(file.to_vec()));
                    }
                }
            }

            // else try to load the entry from http (if active)
            if files.is_none() {
                if let Some((url, hash, ty)) = http_entry.zip(base_path.to_str()).and_then(
                    |((entry, download_url), base_path)| {
                        download_url
                            .join(base_path)
                            .and_then(|url| {
                                url.join(&format!(
                                    "{}_{}.{}",
                                    key.name.as_str(),
                                    fmt_hash(&entry.hash),
                                    entry.ty
                                ))
                            })
                            .map(|url| (url, entry.hash, entry.ty))
                            .ok()
                    },
                ) {
                    if let Ok(file) = http.download_binary(url, &hash).await {
                        if ty == "tar" {
                            if let Ok(tar_files) = read_tar(&file) {
                                files = Some(ContainerLoadedItem::Directory(
                                    ContainerLoadedItemDir::new(tar_files),
                                ));
                            }
                        } else if ty == "png" {
                            files = Some(ContainerLoadedItem::SingleFile(file.to_vec()));
                        }
                    }
                }
            }

            // else try to load from local files without any hash from entry
            if files.is_none() {
                // first try directory
                if let Ok(dir_files) = fs
                    .files_in_dir_recursive(&base_path.join(key.name.as_str()))
                    .await
                {
                    files = Some(ContainerLoadedItem::Directory(ContainerLoadedItemDir::new(
                        dir_files,
                    )));
                }

                // then try tar
                if let Ok(file) = fs
                    .read_file(&base_path.join(format!("{}.tar", key.name.as_str())))
                    .await
                {
                    if let Ok(tar_files) = read_tar(&file) {
                        files = Some(ContainerLoadedItem::Directory(ContainerLoadedItemDir::new(
                            tar_files,
                        )));
                    }
                }

                // then try png (or .ogg etc., which currently are not supported)
                if let Ok(file) = fs
                    .read_file(&base_path.join(format!("{}.png", key.name.as_str())))
                    .await
                {
                    files = Some(ContainerLoadedItem::SingleFile(file.to_vec()));
                }
            }

            match files {
                Some(files) => Ok(files),
                None => Err(anyhow!(
                    "Could not load/download resource with name {} (without hash)",
                    key.name.as_str(),
                )),
            }
        };
        files
    }

    pub fn load_default(io: &Io, base_path: &Path) -> IoBatcherTask<ContainerLoadedItem> {
        let fs = io.fs.clone();
        let http = io.http.clone();
        let base_path = base_path.to_path_buf();

        let container_name_dummy: String = Default::default();
        io.io_batcher.spawn(async move {
            Self::load_container_item(
                container_name_dummy,
                fs,
                http,
                base_path,
                "default".try_into().unwrap(),
                None,
                None,
            )
            .await
        })
    }

    fn load(
        container_name: String,
        graphics_mt: GraphicsMultiThreaded,
        sound_mt: SoundMultiThreaded,
        runtime_thread_pool: &Arc<rayon::ThreadPool>,
        io: &Io,
        base_path: PathBuf,
        key: ContainerKey,
        game_server_http: Option<Url>,
        resource_http_download: Option<(
            TokioArcMutex<Option<anyhow::Result<ContainerItemIndexEntries>>>,
            Url,
        )>,
        default_loaded_item: Arc<ContainerLoadedItemDir>,
    ) -> IoBatcherTask<L> {
        let fs = io.fs.clone();
        let http = io.http.clone();
        let runtime_thread_pool = runtime_thread_pool.clone();

        io.io_batcher.spawn(async move {
            let item_name = key.name.clone();

            let files = Self::load_container_item(
                container_name,
                fs,
                http,
                base_path,
                key,
                game_server_http,
                resource_http_download,
            )
            .await;

            match files {
                Ok(files) => Ok(L::load(
                    item_name.as_str(),
                    files,
                    &default_loaded_item,
                    &runtime_thread_pool,
                    &graphics_mt,
                    &sound_mt,
                )?),
                Err(err) => Err(err),
            }
        })
    }

    /// Get the item for the given key,
    /// or if not exist, try to load it.
    /// Return default as long as the item is loading
    /// or if the item was not found.
    pub fn get_or_default<Q>(&mut self, name: &Q) -> &A
    where
        Q: Borrow<ContainerKey>,
    {
        self.check_default_loaded();

        let item_res = self.items.get(name.borrow());
        if item_res.is_some() {
            let item = self.items.to_back(name.borrow()).unwrap();
            item.used_last_in = self.last_update_time.unwrap_or_default();
            &item.item
        } else {
            // try to load the item
            if let Some(load_item_res) = self.loading_tasks.get_mut(name.borrow()) {
                if let Some(load_item) = load_item_res.take() {
                    if load_item.is_finished() {
                        let loaded_item = load_item.get_storage();
                        match loaded_item {
                            Ok(item) => {
                                let new_item =
                                    item.convert(&self.texture_handle, &self.sound_object_handle);
                                self.items.insert(
                                    name.borrow().clone(),
                                    ContainerItem {
                                        item: new_item,
                                        used_last_in: self.last_update_time.unwrap_or_default(),
                                    },
                                );
                                self.loading_tasks.remove(name.borrow());
                                return &self.items.get(name.borrow()).unwrap().item;
                            }
                            Err(err) => {
                                log::info!(
                                    target: &self.container_name,
                                    "Error while loading item \"{}\": {}",
                                    name.borrow().name.as_str(),
                                    err
                                );
                            }
                        }
                    } else {
                        // put the item back, only remove it when the
                        // task was actually finished
                        let _ = load_item_res.insert(load_item);
                    }
                }
            } else {
                let base_path = self.base_path.clone();
                let key = name.borrow().clone();
                let game_server_http = self.resource_server_download_url.clone();
                let resource_http = self.resource_http_download_url.clone();
                let default_loaded_item = self.default_loaded_item.clone();
                self.loading_tasks.insert(
                    key.clone(),
                    Some(Self::load(
                        self.container_name.clone(),
                        self.graphics_mt.clone(),
                        self.sound_mt.clone(),
                        &self.runtime_thread_pool,
                        &self.io,
                        base_path,
                        key,
                        game_server_http,
                        resource_http.map(|url| (self.resource_http_download_index.clone(), url)),
                        default_loaded_item,
                    )),
                );
            }

            let item = self.items.to_back(&self.default_key).unwrap();
            item.used_last_in = self.last_update_time.unwrap_or_default();
            &item.item
        }
    }

    /// Automatically uses the default key if the given key is `None`,
    /// otherwise identical to [`Container::get_or_default`].
    pub fn get_or_default_opt<Q>(&mut self, name: Option<&Q>) -> &A
    where
        Q: Borrow<ContainerKey>,
    {
        let default_key = self.default_key.clone();
        self.get_or_default(name.map(|name| name.borrow()).unwrap_or(&default_key))
    }

    /// Remove all items and load tasks, except for the default item.
    pub fn clear_except_default(&mut self) {
        self.check_default_loaded();

        let default_item = self.items.remove(&self.default_key).unwrap();
        self.items.clear();
        self.loading_tasks.clear();
        self.items.insert((*self.default_key).clone(), default_item);
    }

    /// Blocking wait for the item to be finished.
    ///
    /// This is only useful for programs that don't run
    /// in real time.
    pub fn blocking_wait_loaded<Q>(&mut self, name: &Q)
    where
        Q: Borrow<ContainerKey>,
    {
        let item_res = self.items.get(name.borrow());
        if item_res.is_none() {
            self.get_or_default(name);
            if let Some(load_item) = self
                .loading_tasks
                .get_mut(name.borrow())
                .and_then(|t| t.as_mut())
            {
                load_item.blocking_wait_finished();
            }
            self.get_or_default(name);
        }
    }

    /// Get a list of entries that can potentially be loaded by this
    /// container.
    /// This also includes skins downloaded over http (if supported/active)
    pub fn entries_index(&mut self) -> HashMap<String, ContainerItemIndexType> {
        let mut entries: HashMap<String, ContainerItemIndexType> = Default::default();
        // do http first so that on collision, disk overwrites the ContainerItemIndexType
        let dir_index = self.resource_http_download_index.blocking_lock();
        if let Some(Ok(dir_index)) = dir_index.as_ref() {
            entries.extend(
                dir_index
                    .iter()
                    .map(|(name, _)| (name.clone(), ContainerItemIndexType::Http)),
            );
        }
        let dir_index = &mut self.resource_dir_index;
        match dir_index {
            Either::Left(dir_index) => {
                if let Ok(dir_index) = dir_index {
                    entries.extend(
                        dir_index
                            .clone()
                            .into_iter()
                            .map(|i| (i, ContainerItemIndexType::Disk)),
                    );
                }
            }
            Either::Right(task) => {
                match task {
                    Some(task) => {
                        if task.is_finished() {
                            let res_dir_index = std::mem::replace(
                                &mut self.resource_dir_index,
                                Either::Right(None),
                            );
                            if let Either::Right(Some(task)) = res_dir_index {
                                let res_dir_index = task.get_storage().ok().unwrap_or_else(|| {
                                    Err(anyhow!("get entries in dir task failed."))
                                });
                                self.resource_dir_index = Either::Left(res_dir_index);
                            }
                        }
                    }
                    None => {
                        // load index
                        let fs = self.io.fs.clone();
                        let path = self.base_path.clone();
                        let container_name = self.container_name.clone();
                        let task = self.io.io_batcher.spawn(async move {
                            let entries = fs.entries_in_dir(&path).await;

                            if let Err(err) = &entries {
                                info!(target: &container_name,
                                    "failed to create index for {container_name}: {err}");
                            }

                            let entries = entries.map(|mut entries| {
                                // filter entries that end with an hash
                                entries.retain(|entry, _| {
                                    let entry: &Path = entry.as_ref();
                                    if let Some((_, name_hash)) = entry
                                        .file_stem()
                                        .and_then(|s| s.to_str())
                                        .and_then(|s| s.rsplit_once('_'))
                                    {
                                        if name_hash.len() == Hash::default().len()
                                            && name_hash
                                                .find(|c: char| !c.is_ascii_hexdigit())
                                                .is_none()
                                        {
                                            return false;
                                        }
                                    }
                                    true
                                });
                                entries
                                    .keys()
                                    .map(|entry| {
                                        let entry: &Path = entry.as_ref();
                                        entry
                                            .file_stem()
                                            .map(|s| s.to_string_lossy().to_string())
                                            .unwrap_or_default()
                                    })
                                    .collect()
                            });

                            Ok(entries)
                        });

                        *dir_index = Either::Right(Some(task));
                    }
                }
            }
        }
        entries
    }
}

pub struct DataFilePartResult<'a> {
    data: &'a Vec<u8>,
    /// Was loaded by the default fallback mechanism
    from_default: bool,
}

/// helper functions the containers can use to quickly load
/// one part or if not existing, the default part
pub fn load_file_part<'a>(
    files: &'a ContainerLoadedItemDir,
    default_files: &'a ContainerLoadedItemDir,
    item_name: &str,
    extra_paths: &[&str],
    part_name: &str,
    allow_default: bool,
) -> anyhow::Result<DataFilePartResult<'a>> {
    let mut part_full_path = PathBuf::new();
    extra_paths.iter().for_each(|extra_path| {
        part_full_path.push(extra_path);
    });
    part_full_path.push(part_name);
    part_full_path.set_extension("png");

    let is_default = item_name == "default";

    let file = files.files.get(&part_full_path);

    match file {
        None => {
            if !is_default && allow_default {
                // try to load default part instead
                let mut png_path_def = PathBuf::new();
                extra_paths.iter().for_each(|extra_path| {
                    png_path_def.push(extra_path);
                });
                png_path_def.push(part_name);
                png_path_def.set_extension("png");
                let file_def = default_files.files.get(&png_path_def);
                if let Some(file_def) = file_def {
                    Ok(DataFilePartResult {
                        data: file_def,
                        from_default: true,
                    })
                } else {
                    Err(anyhow!(
                        "default asset part (".to_string() + part_name + ") not found"
                    ))
                }
            } else {
                Err(anyhow!(
                    "default asset part ({}) not found in {:?}",
                    part_name,
                    part_full_path,
                ))
            }
        }
        Some(file) => Ok(DataFilePartResult {
            data: file,
            from_default: false,
        }),
    }
}

pub struct PngFilePartResult {
    pub png: PngResultPersistent,
    /// Was loaded by the default fallback mechanism
    pub from_default: bool,
}

pub fn load_file_part_as_png(
    files: &ContainerLoadedItemDir,
    default_files: &ContainerLoadedItemDir,
    item_name: &str,
    extra_paths: &[&str],
    part_name: &str,
) -> anyhow::Result<PngFilePartResult> {
    load_file_part_as_png_ex(
        files,
        default_files,
        item_name,
        extra_paths,
        part_name,
        true,
    )
}

pub fn load_file_part_as_png_ex(
    files: &ContainerLoadedItemDir,
    default_files: &ContainerLoadedItemDir,
    item_name: &str,
    extra_paths: &[&str],
    part_name: &str,
    allow_default: bool,
) -> anyhow::Result<PngFilePartResult> {
    let file = load_file_part(
        files,
        default_files,
        item_name,
        extra_paths,
        part_name,
        allow_default,
    )?;
    let mut img_data = Vec::<u8>::new();
    let part_img = load_png_image(file.data, |width, height, bytes_per_pixel| {
        img_data = vec![0; width * height * bytes_per_pixel];
        &mut img_data
    })?;
    Ok(PngFilePartResult {
        png: part_img.prepare_moved_persistent().to_persistent(img_data),
        from_default: file.from_default,
    })
}

pub struct ImgFilePartResult {
    pub img: ContainerItemLoadData,
    /// Was loaded by the default fallback mechanism
    pub from_default: bool,
}

pub fn load_file_part_and_upload(
    graphics_mt: &GraphicsMultiThreaded,
    files: &ContainerLoadedItemDir,
    default_files: &ContainerLoadedItemDir,
    item_name: &str,
    extra_paths: &[&str],
    part_name: &str,
) -> anyhow::Result<ImgFilePartResult> {
    load_file_part_and_upload_ex(
        graphics_mt,
        files,
        default_files,
        item_name,
        extra_paths,
        part_name,
        true,
    )
}

pub fn load_file_part_and_upload_ex(
    graphics_mt: &GraphicsMultiThreaded,
    files: &ContainerLoadedItemDir,
    default_files: &ContainerLoadedItemDir,
    item_name: &str,
    extra_paths: &[&str],
    part_name: &str,
    allow_default: bool,
) -> anyhow::Result<ImgFilePartResult> {
    let part_img = load_file_part_as_png_ex(
        files,
        default_files,
        item_name,
        extra_paths,
        part_name,
        allow_default,
    )?;
    let mut img = graphics_mt.mem_alloc(GraphicsMemoryAllocationType::Texture {
        width: part_img.png.width as usize,
        height: part_img.png.height as usize,
        depth: 1,
        is_3d_tex: false,
        flags: TexFlags::empty(),
    });
    img.as_mut_slice().copy_from_slice(&part_img.png.data);
    if let Err(err) = graphics_mt.try_flush_mem(&mut img, true) {
        // Ignore the error, but log it.
        log::debug!("err while flushing memory: {} for {part_name}", err);
    }
    Ok(ImgFilePartResult {
        img: ContainerItemLoadData {
            width: part_img.png.width,
            height: part_img.png.height,
            depth: 1,
            data: img,
        },
        from_default: part_img.from_default,
    })
}

pub struct SoundFilePartResult {
    pub mem: SoundBackendMemory,
    /// Was loaded by the default fallback mechanism
    pub from_default: bool,
}

pub fn load_sound_file_part_and_upload(
    sound_mt: &SoundMultiThreaded,
    files: &ContainerLoadedItemDir,
    default_files: &ContainerLoadedItemDir,
    item_name: &str,
    extra_paths: &[&str],
    part_name: &str,
) -> anyhow::Result<SoundFilePartResult> {
    load_sound_file_part_and_upload_ex(
        sound_mt,
        files,
        default_files,
        item_name,
        extra_paths,
        part_name,
        true,
    )
}

pub fn load_sound_file_part_and_upload_ex(
    sound_mt: &SoundMultiThreaded,
    files: &ContainerLoadedItemDir,
    default_files: &ContainerLoadedItemDir,
    item_name: &str,
    extra_paths: &[&str],
    part_name: &str,
    allow_default: bool,
) -> anyhow::Result<SoundFilePartResult> {
    let mut sound_path = PathBuf::new();

    for extra_path in extra_paths {
        sound_path = sound_path.join(Path::new(extra_path));
    }

    sound_path = sound_path.join(Path::new(&format!("{}.ogg", part_name)));

    let is_default = item_name == "default";

    let (file, from_default) = match files.files.get(&sound_path) {
        Some(file) => Ok((file, false)),
        None => {
            if !is_default && allow_default {
                // try to load default part instead
                let mut path_def = PathBuf::new();
                extra_paths.iter().for_each(|extra_path| {
                    path_def.push(extra_path);
                });
                path_def.push(part_name);
                path_def.set_extension("ogg");
                default_files
                    .files
                    .get(&path_def)
                    .ok_or_else(|| {
                        anyhow!(
                            "requested sound file {} didn't exist in default items",
                            item_name
                        )
                    })
                    .map(|s| (s, true))
            } else {
                Err(anyhow!(
                    "requested sound file for {} not found: {}",
                    item_name,
                    part_name
                ))
            }
        }
    }?;

    let mut mem = sound_mt.mem_alloc(file.len());
    mem.as_mut_slice().copy_from_slice(file);
    if let Err(err) = sound_mt.try_flush_mem(&mut mem) {
        // Ignore the error, but log it.
        log::debug!("err while flushing memory: {} for {part_name}", err);
    }

    Ok(SoundFilePartResult { mem, from_default })
}

/// returns the png data, the width and height are the 3d texture w & h, additionally the depth is returned
pub fn load_file_part_as_png_and_convert_3d(
    runtime_thread_pool: &Arc<rayon::ThreadPool>,
    files: &ContainerLoadedItemDir,
    default_files: &ContainerLoadedItemDir,
    item_name: &str,
    extra_paths: &[&str],
    part_name: &str,
) -> anyhow::Result<(PngFilePartResult, usize)> {
    let file = load_file_part(
        files,
        default_files,
        item_name,
        extra_paths,
        part_name,
        true,
    )?;
    let mut img_data = Vec::<u8>::new();
    let part_img = load_png_image(file.data, |width, height, bytes_per_pixel| {
        img_data = vec![0; width * height * bytes_per_pixel];
        &mut img_data
    })?;

    let mut part_img = part_img.prepare_moved_persistent().to_persistent(img_data);

    let mut tex_3d: Vec<u8> = Vec::new();
    tex_3d.resize(
        part_img.width as usize * part_img.height as usize * 4,
        Default::default(),
    );
    let mut image_3d_width = 0;
    let mut image_3d_height = 0;
    if !texture_2d_to_3d(
        runtime_thread_pool,
        &part_img.data,
        part_img.width as usize,
        part_img.height as usize,
        4,
        16,
        16,
        tex_3d.as_mut_slice(),
        &mut image_3d_width,
        &mut image_3d_height,
    ) {
        Err(anyhow!("error while converting entities to 3D"))?
    }

    part_img.width = image_3d_width as u32;
    part_img.height = image_3d_height as u32;
    part_img.data = tex_3d;

    Ok((
        PngFilePartResult {
            png: part_img,
            from_default: file.from_default,
        },
        16 * 16,
    ))
}

pub fn load_file_part_and_convert_3d_and_upload(
    graphics_mt: &GraphicsMultiThreaded,
    runtime_thread_pool: &Arc<rayon::ThreadPool>,
    files: &ContainerLoadedItemDir,
    default_files: &ContainerLoadedItemDir,
    item_name: &str,
    extra_paths: &[&str],
    part_name: &str,
) -> anyhow::Result<ImgFilePartResult> {
    let (part_img, depth) = load_file_part_as_png_and_convert_3d(
        runtime_thread_pool,
        files,
        default_files,
        item_name,
        extra_paths,
        part_name,
    )?;
    let mut img = graphics_mt.mem_alloc(GraphicsMemoryAllocationType::Texture {
        width: part_img.png.width as usize,
        height: part_img.png.height as usize,
        depth,
        is_3d_tex: true,
        flags: TexFlags::empty(),
    });
    img.as_mut_slice().copy_from_slice(&part_img.png.data);
    if let Err(err) = graphics_mt.try_flush_mem(&mut img, true) {
        // Ignore the error, but log it.
        log::debug!("err while flushing memory: {} for {part_name}", err);
    }
    Ok(ImgFilePartResult {
        img: ContainerItemLoadData {
            width: part_img.png.width,
            height: part_img.png.height,
            depth: depth as u32,
            data: img,
        },
        from_default: part_img.from_default,
    })
}
