use std::{collections::HashMap, sync::Arc};

use anyhow::anyhow;
use arrayvec::ArrayString;
use async_trait::async_trait;
use base_io_traits::fs_traits::FileSystemInterface;

use base_io::{io::IO, io_batcher::IOBatcherTask};
use base_log::log::{LogLevel, SystemLog, SystemLogGroup, SystemLogInterface};
use graphics::{
    graphics::{Graphics, GraphicsTextureHandle},
    graphics_mt::GraphicsMultiThreaded,
    image::texture_2d_to_3d,
};
use graphics_types::{
    commands::TexFlags,
    types::{GraphicsBackendMemory, GraphicsMemoryAllocationType},
};
use hashlink::LinkedHashMap;
use hiarc_macro::Hiarc;
use image::png::{load_png_image, PngResultPersistent};

#[derive(Debug)]
pub struct ContainerItemLoadData {
    pub width: u32,
    pub height: u32,
    pub depth: u32,
    pub data: GraphicsBackendMemory,
}

#[derive(Debug)]
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
    #[hiarc]
    texture_handle: GraphicsTextureHandle,
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
    ) -> anyhow::Result<Self>;

    fn convert(self, texture_handle: &GraphicsTextureHandle) -> A;
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
            runtime_thread_pool,
            logger: log.logger(container_name),
        }
    }

    pub fn update(&mut self) {
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
        item_name: &str,
        io: &IO,
        runtime_thread_pool: &Arc<rayon::ThreadPool>,
    ) -> IOBatcherTask<L> {
        let fs = io.fs.clone();
        let item_name = item_name.to_string();

        let runtime_thread_pool = runtime_thread_pool.clone();

        io.io_batcher.spawn(async move {
            L::load(&item_name, &fs, &runtime_thread_pool, &graphics_mt).await
        })
    }

    pub fn get_or_default(&mut self, name: &str) -> &A {
        // make sure default is loaded
        if let Some(default_item) = self.default_item.take() {
            self.items.insert(
                "default".to_string(),
                ContainerItem {
                    item: default_item
                        .get_storage()
                        .unwrap()
                        .convert(&self.texture_handle),
                    used_last_in_update: 0,
                },
            );
        }

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
                                let new_item = item.convert(&self.texture_handle);
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
    collection_path: &ArrayString<4096>,
    item_name: &str,
    extra_paths: &[&str],
    part_name: &str,
) -> anyhow::Result<Vec<u8>> {
    let mut part_full_path = *collection_path;
    part_full_path.push_str(item_name);
    part_full_path.push_str("/");
    extra_paths.iter().for_each(|extra_path| {
        part_full_path.push_str(extra_path);
        part_full_path.push_str("/");
    });
    part_full_path.push_str(part_name);
    part_full_path.push_str(".png");

    let is_default = item_name == "default";

    let file = fs.open_file(part_full_path.as_str()).await;

    match file {
        Err(err) => {
            if !is_default {
                // try to load default part instead
                let mut skin_path_def = *collection_path;
                skin_path_def.push_str("default");
                skin_path_def.push_str("/");
                extra_paths.iter().for_each(|extra_path| {
                    skin_path_def.push_str(extra_path);
                    skin_path_def.push_str("/");
                });
                skin_path_def.push_str(part_name);
                skin_path_def.push_str(".png");
                let file_def = fs.open_file(skin_path_def.as_str()).await;
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
                    "default asset part (".to_string()
                        + part_name
                        + ") not found in \""
                        + part_full_path.as_str()
                        + "\": "
                        + &err.to_string()
                ))
            }
        }
        Ok(file) => Ok(file),
    }
}

pub async fn load_file_part_as_png(
    fs: &dyn FileSystemInterface,
    collection_path: &ArrayString<4096>,
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
    collection_path: &ArrayString<4096>,
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

/// returns the png data, the width and height are the 3d texture w & h, additionally the depth is returned
pub async fn load_file_part_as_png_and_convert_3d(
    fs: &dyn FileSystemInterface,
    runtime_thread_pool: &Arc<rayon::ThreadPool>,
    collection_path: &ArrayString<4096>,
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
    collection_path: &ArrayString<4096>,
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
