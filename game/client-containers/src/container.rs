use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::anyhow;
use async_trait::async_trait;
use base_io_traits::fs_traits::FileSystemInterface;

use base_io::{io::IO, io_batcher::IOBatcherTask};
use base_log::log::{LogLevel, SystemLog, SystemLogGroup, SystemLogInterface};
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
use sound::{
    scene_object::SceneObject, sound::SoundManager, sound_handle::SoundObjectHandle,
    sound_mt::SoundMultiThreaded, sound_mt_types::SoundBackendMemory,
};

#[derive(Debug, Hiarc)]
pub struct ContainerItemLoadData {
    pub width: u32,
    pub height: u32,
    pub depth: u32,
    pub data: GraphicsBackendMemory,
}

#[derive(Debug, Hiarc)]
struct ContainerItem<A> {
    item: A,
    used_last_in_update: usize,
}

/// Containers are a collection of named assets, e.g. all skins
/// are part of the skins container. Skins have a name and corresponding to this name
/// there are textures, sounds, effects or whatever fits the container logically
/// All containers should have a `default` value/texture/sound etc.
#[derive(Debug, Hiarc)]
pub struct Container<A, L> {
    items: LinkedHashMap<String, ContainerItem<A>>,
    update_count: usize,
    loading_tasks: HashMap<String, Option<IOBatcherTask<L>>>,

    // containers allow to delay loading the default item as much as possible, to improve startup time
    default_item: Option<IOBatcherTask<L>>,

    // strict private data
    io: IO,
    graphics_mt: GraphicsMultiThreaded,
    texture_handle: GraphicsTextureHandle,
    sound_mt: SoundMultiThreaded,
    sound_object_handle: SoundObjectHandle,
    #[hiarc_skip_unsafe]
    runtime_thread_pool: Arc<rayon::ThreadPool>,
    logger: SystemLogGroup,
}

#[async_trait]
pub trait ContainerLoad<A>
where
    Self: Sized,
{
    async fn load(
        item_name: &str,
        fs: &Arc<dyn FileSystemInterface>,
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
    pub fn new(
        io: IO,
        runtime_thread_pool: Arc<rayon::ThreadPool>,
        default_item: IOBatcherTask<L>,
        log: &SystemLog,
        container_name: &str,
        graphics: &Graphics,
        sound: &SoundManager,
        sound_scene: &SceneObject,
    ) -> Self {
        let items = LinkedHashMap::new();
        Self {
            items,
            update_count: 0,
            loading_tasks: HashMap::new(),

            default_item: Some(default_item),

            io,
            graphics_mt: graphics.get_graphics_mt(),
            texture_handle: graphics.texture_handle.clone(),
            sound_mt: sound.get_sound_mt(),
            sound_object_handle: sound_scene.sound_object_handle.clone(),
            runtime_thread_pool,
            logger: log.logger(container_name),
        }
    }

    fn check_default_loaded(&mut self) {
        // make sure default is loaded
        if let Some(default_item) = self.default_item.take() {
            self.items.insert(
                "default".to_string(),
                ContainerItem {
                    item: default_item
                        .get_storage()
                        .unwrap()
                        .convert(&self.texture_handle, &self.sound_object_handle),
                    used_last_in_update: 0,
                },
            );
        }
    }

    pub fn update<'a>(&mut self, force_used_items: impl Iterator<Item = &'a str>) {
        self.check_default_loaded();

        // make sure these entries are always kept loaded
        for force_used_item in force_used_items {
            if let Some(item) = self.items.to_back(force_used_item) {
                item.used_last_in_update = self.update_count;
            }
        }

        // all items that were not used lately
        // are always among the first items
        // delete them if they were not used lately
        while !self.items.is_empty() {
            let (name, item) = self.items.iter_mut().next().unwrap();
            if item.used_last_in_update + 10 /* TODO!: RANDOM value */ < self.update_count
                && name != "default"
            {
                let name_clone = name.clone();
                let _ = self.items.remove(&name_clone).unwrap();
            } else {
                break;
            }
        }
        self.update_count += 1;
        let item = self.items.to_back("default").unwrap();
        item.used_last_in_update = self.update_count;
    }

    pub fn load(
        graphics_mt: GraphicsMultiThreaded,
        sound_mt: SoundMultiThreaded,
        item_name: &str,
        io: &IO,
        runtime_thread_pool: &Arc<rayon::ThreadPool>,
    ) -> IOBatcherTask<L> {
        let fs = io.fs.clone();
        let item_name = item_name.to_string();

        let runtime_thread_pool = runtime_thread_pool.clone();

        io.io_batcher.spawn(async move {
            L::load(
                &item_name,
                &fs,
                &runtime_thread_pool,
                &graphics_mt,
                &sound_mt,
            )
            .await
        })
    }

    pub fn get_or_default(&mut self, name: &str) -> &A {
        self.check_default_loaded();

        let item_res = self.items.get(name);
        if item_res.is_some() {
            let item = self.items.to_back(name).unwrap();
            item.used_last_in_update = self.update_count;
            &item.item
        } else {
            // try to load the item
            if let Some(load_item_res) = self.loading_tasks.get_mut(name) {
                if let Some(load_item) = load_item_res.take() {
                    if load_item.is_finished() {
                        let loaded_item = load_item.get_storage();
                        match loaded_item {
                            Ok(item) => {
                                let new_item =
                                    item.convert(&self.texture_handle, &self.sound_object_handle);
                                self.items.insert(
                                    name.to_string(),
                                    ContainerItem {
                                        item: new_item,
                                        used_last_in_update: self.update_count,
                                    },
                                );
                                self.loading_tasks.remove(name);
                                return &self.items.get(name).unwrap().item;
                            }
                            Err(err) => {
                                self.logger.log(LogLevel::Error).msg(&format!(
                                    "Error while loading item \"{}\": {}",
                                    name, err
                                ));
                            }
                        }
                    } else {
                        // put the item back, only remove it when the
                        // task was actually finished
                        let _ = load_item_res.insert(load_item);
                    }
                }
            } else {
                self.loading_tasks.insert(
                    name.to_string(),
                    Some(Self::load(
                        self.graphics_mt.clone(),
                        self.sound_mt.clone(),
                        name,
                        &self.io,
                        &self.runtime_thread_pool,
                    )),
                );
            }

            let item = self.items.to_back("default").unwrap();
            item.used_last_in_update = self.update_count;
            &item.item
        }
    }
}

/// helper functions the containers can use to quickly load
/// one part or if not existing, the default part
pub async fn load_file_part(
    fs: &dyn FileSystemInterface,
    collection_path: &Path,
    item_name: &str,
    extra_paths: &[&str],
    part_name: &str,
) -> anyhow::Result<Vec<u8>> {
    let mut part_full_path = PathBuf::from(collection_path);
    part_full_path.push(item_name);
    extra_paths.iter().for_each(|extra_path| {
        part_full_path.push(extra_path);
    });
    part_full_path.push(part_name);
    part_full_path.set_extension("png");

    let is_default = item_name == "default";

    let file = fs.open_file(&part_full_path).await;

    match file {
        Err(err) => {
            if !is_default {
                // try to load default part instead
                let mut png_path_def = PathBuf::from(collection_path);
                png_path_def.push("default");
                extra_paths.iter().for_each(|extra_path| {
                    png_path_def.push(extra_path);
                });
                png_path_def.push(part_name);
                png_path_def.set_extension("png");
                let file_def = fs.open_file(&png_path_def).await;
                if let Err(err) = file_def {
                    Err(anyhow!(
                        "default asset part (".to_string()
                            + part_name
                            + ") not found: "
                            + &err.to_string()
                    ))
                } else {
                    Ok(file_def.unwrap())
                }
            } else {
                Err(anyhow!(
                    "default asset part ({}) not found in \"{:?}\": {}",
                    part_name,
                    part_full_path,
                    err
                ))
            }
        }
        Ok(file) => Ok(file),
    }
}

pub async fn load_file_part_as_png(
    fs: &dyn FileSystemInterface,
    collection_path: &Path,
    item_name: &str,
    extra_paths: &[&str],
    part_name: &str,
) -> anyhow::Result<PngResultPersistent> {
    let file = load_file_part(fs, collection_path, item_name, extra_paths, part_name).await?;
    let mut img_data = Vec::<u8>::new();
    let part_img = load_png_image(&file, |width, height, bytes_per_pixel| {
        img_data = vec![0; width * height * bytes_per_pixel];
        &mut img_data
    })?;
    Ok(part_img.prepare_moved_persistent().to_persistent(img_data))
}

pub async fn load_file_part_and_upload(
    graphics_mt: &GraphicsMultiThreaded,
    fs: &dyn FileSystemInterface,
    collection_path: &Path,
    item_name: &str,
    extra_paths: &[&str],
    part_name: &str,
) -> anyhow::Result<ContainerItemLoadData> {
    let part_img =
        load_file_part_as_png(fs, collection_path, item_name, extra_paths, part_name).await?;
    let mut img = graphics_mt.mem_alloc(GraphicsMemoryAllocationType::Texture {
        width: part_img.width as usize,
        height: part_img.height as usize,
        depth: 1,
        is_3d_tex: false,
        flags: TexFlags::empty(),
    });
    img.as_mut_slice().copy_from_slice(&part_img.data);
    if graphics_mt.try_flush_mem(&mut img, true).is_err() {
        // TODO: ignore?
    }
    Ok(ContainerItemLoadData {
        width: part_img.width,
        height: part_img.height,
        depth: 1,
        data: img,
    })
}

pub async fn load_sound_file_part_and_upload(
    sound_mt: &SoundMultiThreaded,
    fs: &dyn FileSystemInterface,
    collection_path: &Path,
    item_name: &str,
    extra_paths: &[&str],
    part_name: &str,
) -> anyhow::Result<SoundBackendMemory> {
    let mut weapon_path = collection_path.join(Path::new(item_name));

    for extra_path in extra_paths {
        weapon_path = weapon_path.join(Path::new(extra_path));
    }

    weapon_path = weapon_path.join(Path::new(&format!("{}.ogg", part_name)));

    let is_default = item_name == "default";

    let file = match fs.open_file(weapon_path.as_ref()).await {
        Ok(file) => Ok(file),
        Err(err) => {
            if !is_default {
                // try to load default part instead
                let mut path_def = PathBuf::from(collection_path);
                path_def.push("default");
                extra_paths.iter().for_each(|extra_path| {
                    path_def.push(extra_path);
                });
                path_def.push(part_name);
                path_def.set_extension("ogg");
                fs.open_file(&path_def).await
            } else {
                Err(err)
            }
        }
    }?;

    let mut img = sound_mt.mem_alloc(file.len());
    img.as_mut_slice().copy_from_slice(&file);
    if sound_mt.try_flush_mem(&mut img).is_err() {
        // TODO: ignore?
    }
    Ok(img)
}

/// returns the png data, the width and height are the 3d texture w & h, additionally the depth is returned
pub async fn load_file_part_as_png_and_convert_3d(
    fs: &dyn FileSystemInterface,
    runtime_thread_pool: &Arc<rayon::ThreadPool>,
    collection_path: &Path,
    item_name: &str,
    extra_paths: &[&str],
    part_name: &str,
) -> anyhow::Result<(PngResultPersistent, usize)> {
    let file = load_file_part(fs, collection_path, item_name, extra_paths, part_name).await?;
    let mut img_data = Vec::<u8>::new();
    let part_img = load_png_image(&file, |width, height, bytes_per_pixel| {
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

    Ok((part_img, 16 * 16))
}

pub async fn load_file_part_and_convert_3d_and_upload(
    graphics_mt: &GraphicsMultiThreaded,
    fs: &dyn FileSystemInterface,
    runtime_thread_pool: &Arc<rayon::ThreadPool>,
    collection_path: &Path,
    item_name: &str,
    extra_paths: &[&str],
    part_name: &str,
) -> anyhow::Result<ContainerItemLoadData> {
    let (part_img, depth) = load_file_part_as_png_and_convert_3d(
        fs,
        runtime_thread_pool,
        collection_path,
        item_name,
        extra_paths,
        part_name,
    )
    .await?;
    let mut img = graphics_mt.mem_alloc(GraphicsMemoryAllocationType::Texture {
        width: part_img.width as usize,
        height: part_img.height as usize,
        depth,
        is_3d_tex: true,
        flags: TexFlags::empty(),
    });
    img.as_mut_slice().copy_from_slice(&part_img.data);
    if graphics_mt.try_flush_mem(&mut img, true).is_err() {
        // TODO: ignore?
    }
    Ok(ContainerItemLoadData {
        width: part_img.width,
        height: part_img.height,
        depth: depth as u32,
        data: img,
    })
}
