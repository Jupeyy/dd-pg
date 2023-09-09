use std::{collections::HashMap, sync::Arc};

use anyhow::anyhow;
use arrayvec::ArrayString;
use async_trait::async_trait;
use base_fs::{
    filesys::FileSystem,
    io_batcher::{TokIOBatcher, TokIOBatcherTask},
};
use base_fs_traits::traits::FileSystemInterface;

use graphics::image::texture_2d_to_3d;
use graphics_backend::types::Graphics;
use hashlink::LinkedHashMap;
use image::png::{load_png_image, PngResultPersistent};

struct ContainerItem<A> {
    item: A,
    used_last_in_update: usize,
}

/**
 * Containers are a collection of named assets, e.g. all skins
 * are part of the skins container. Skins have a name and corresponding to this name
 * there are textures, sounds, effects or whatever fits the container logically
 * All containers should have a `default` value/texture/sound etc.
 */
pub struct Container<A, L> {
    items: LinkedHashMap<String, ContainerItem<A>>,
    update_count: usize,
    loading_tasks: HashMap<String, Option<TokIOBatcherTask<L>>>,

    // containers allow to delay loading the default item as much as possible, to improve startup time
    default_item: Option<TokIOBatcherTask<L>>,
}

#[async_trait]
pub trait ContainerLoad<A> {
    async fn load(
        &mut self,
        item_name: &str,
        fs: &Arc<FileSystem>,
        runtime_thread_pool: &Arc<rayon::ThreadPool>,
    ) -> anyhow::Result<()>;

    fn convert(self, graphics: &mut Graphics) -> A;
}

impl<A, L> Container<A, L>
where
    L: Default + ContainerLoad<A> + Sync + Send + 'static,
{
    pub fn new(default_item: TokIOBatcherTask<L>) -> Self {
        let items = LinkedHashMap::new();
        Self {
            items,
            update_count: 0,
            loading_tasks: HashMap::new(),

            default_item: Some(default_item),
        }
    }

    pub fn update(&mut self) {
        // all items that were not used lately
        // are always among the first items
        // delete them if they were not used lately
        while !self.items.is_empty() {
            let (name, item) = self.items.iter_mut().next().unwrap();
            if name != "default"
                && item.used_last_in_update + 10 /* TODO!: RANDOM value */ < self.update_count
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
        item_name: &str,
        fs: &Arc<FileSystem>,
        io_batcher: &TokIOBatcher,
        runtime_thread_pool: Arc<rayon::ThreadPool>,
    ) -> TokIOBatcherTask<L> {
        let fs = fs.clone();
        let item_name = item_name.to_string();

        io_batcher.spawn(async move {
            let mut storage = L::default();
            let def_skin = storage.load(&item_name, &fs, &runtime_thread_pool).await;
            if let Err(err) = def_skin {
                Err(anyhow!("{}", err))
            } else {
                Ok(storage)
            }
        })
    }

    pub fn get_or_default(
        &mut self,
        name: &str,
        graphics: &mut Graphics,
        fs: &Arc<FileSystem>,
        io_batcher: &TokIOBatcher,
        runtime_thread_pool: &Arc<rayon::ThreadPool>,
    ) -> &A {
        // make sure default is loaded
        if let Some(default_item) = self.default_item.take() {
            self.items.insert(
                "default".to_string(),
                ContainerItem {
                    item: default_item.get_storage().unwrap().convert(graphics),
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
                                let new_item = item.convert(graphics);
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
                                println!(
                                    "Error while loading item \"{}\": {}",
                                    name,
                                    err.to_string()
                                )
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
                        name,
                        fs,
                        io_batcher,
                        runtime_thread_pool.clone(),
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
    fs: &FileSystem,
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
                            + &part_name.to_string()
                            + &") not found: ".to_string()
                            + &err.to_string()
                    ))
                } else {
                    Ok(file_def.unwrap())
                }
            } else {
                Err(anyhow!(
                    "default asset part (".to_string()
                        + &part_name.to_string()
                        + &") not found in \""
                        + part_full_path.as_str()
                        + &"\": ".to_string()
                        + &err.to_string()
                ))
            }
        }
        Ok(file) => Ok(file),
    }
}

pub async fn load_file_part_as_png(
    fs: &FileSystem,
    collection_path: &ArrayString<4096>,
    item_name: &str,
    extra_paths: &[&str],
    part_name: &str,
) -> anyhow::Result<PngResultPersistent> {
    let file = load_file_part(fs, collection_path, item_name, extra_paths, part_name).await?;
    let mut img_data = Vec::<u8>::new();
    let part_img = load_png_image(&file, |size| {
        img_data = vec![0; size];
        &mut img_data
    })?;
    Ok(part_img.to_persistent())
}

pub async fn load_file_part_as_png_and_convert_3d(
    fs: &FileSystem,
    runtime_thread_pool: &Arc<rayon::ThreadPool>,
    collection_path: &ArrayString<4096>,
    item_name: &str,
    extra_paths: &[&str],
    part_name: &str,
) -> anyhow::Result<PngResultPersistent> {
    let file = load_file_part(fs, collection_path, item_name, extra_paths, part_name).await?;
    let mut img_data = Vec::<u8>::new();
    let part_img = load_png_image(&file, |size| {
        img_data = vec![0; size];
        &mut img_data
    })?;

    let mut part_img = part_img.to_persistent();

    let mut tex_3d: Vec<u8> = Vec::new();
    tex_3d.resize(
        part_img.width as usize * part_img.height as usize * 4,
        Default::default(),
    );
    let mut image_3d_width = 0 as usize;
    let mut image_3d_height = 0 as usize;
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

    Ok(part_img)
}