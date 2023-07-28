use std::{collections::HashMap, sync::Arc};

use config::config::Config;
use rayon::prelude::{IndexedParallelIterator, IntoParallelRefMutIterator, ParallelIterator};

use graphics::image::{highest_bit, resize, texture_2d_to_3d};

use base::{benchmark, system::SystemTimeInterface};
use base_fs::{
    filesys::FileSystem,
    io_batcher::{TokIOBatcher, TokIOBatcherTask},
};
use shared::game::state_wasm_manager::GameStateWasmManager;
use shared_game::{collision::collision::Collision, state::state::GameStateCreatePipe};

use crate::{
    client::image::png::load_png_image,
    client_map_buffered::{ClientMapBufferUploadData, ClientMapBuffered},
};

use shared_base::{
    datafile::{
        CDatafileWrapper, MapFileImageReadOptions, MapFileLayersReadOptions, MapFileOpenOptions,
        ReadFile,
    },
    join_all,
    mapdef::{MapImage, MapLayer},
};

use graphics::graphics::Graphics;

use graphics_types::{
    command_buffer::TexFlags,
    rendering::TextureIndex,
    types::ImageFormat,
    types::{GraphicsBackendMemory, GraphicsMemoryAllocationType},
};

use super::map::RenderMap;

pub struct ClientMapFileProcessed {
    pub raw: CDatafileWrapper,
    pub render: RenderMap,
    pub upload_data: ClientMapBufferUploadData,
    pub collision: Collision,
}

pub struct ClientMapLoadingFile {
    pub task: TokIOBatcherTask<ClientMapFileProcessed>,
}

pub struct ClientMapImage {
    pub texture_index: Option<TextureIndex>,
    pub texture_index_3d: Option<TextureIndex>,
}

pub struct ClientMapFileData {
    pub raw: CDatafileWrapper,
    pub images: Vec<ClientMapImage>,
    pub render: RenderMap,
    pub collision: Collision,
    pub buffered_map: ClientMapBuffered,
}

pub struct ClientMapFile {
    pub data: ClientMapFileData,
    // client local calculated game
    pub game: GameStateWasmManager,
}

impl ClientMapFile {
    pub fn new(
        thread_pool: &Arc<rayon::ThreadPool>,
        map_file: &str,
        io_batcher: &Arc<std::sync::Mutex<TokIOBatcher>>,
        graphics: &mut Graphics,
        fs: &Arc<FileSystem>,
        config: &Config,
        sys: &Arc<impl SystemTimeInterface + Send + Sync + 'static>,
    ) -> ClientMapLoadingFile {
        let map_file = "maps/".to_string() + &map_file;
        let map_file_name = map_file.clone() + &".map";
        let file_system = fs.clone();
        let do_benchmark = config.dbg_bench;
        let sys_time = sys.clone();
        let runtime_tp = thread_pool.clone();
        let graphics_mt = graphics.get_graphics_mt();
        ClientMapLoadingFile {
            task: io_batcher.lock().as_mut().unwrap().spawn(async move {
                tokio::task::spawn_blocking(move || {
                    // Load the map file
                    let fs_clone = file_system.clone();

                    let file = tokio::task::block_in_place(|| {
                        tokio::runtime::Handle::current()
                            .block_on(fs_clone.open_file(map_file_name.as_str()))
                    })
                    .unwrap();

                    // open the map file
                    let mut file_wrap = CDatafileWrapper::new();
                    let mut data_start: &[u8] = &[];
                    benchmark!(do_benchmark, &sys_time, "opening the full map file", || {
                        let res = file_wrap.open(
                            &file,
                            map_file.as_str(),
                            runtime_tp.as_ref(),
                            &MapFileOpenOptions {
                                do_benchmark,
                                ..Default::default()
                            },
                            &sys_time,
                        );

                        if let Ok(data_start_res) = res {
                            data_start = data_start_res;
                        }
                    });

                    // read content files
                    let mut read_files: HashMap<String, ReadFile> = HashMap::new();
                    std::mem::swap(&mut read_files, &mut file_wrap.read_files);
                    let task_read = tokio::spawn(async move {
                        for (read_file_path, read_file_info) in &mut read_files {
                            let file_res = file_system.open_file(read_file_path).await;
                            if let Ok(file) = file_res {
                                match read_file_info {
                                    shared_base::datafile::ReadFile::Image(
                                        _image_index,
                                        img_file_data,
                                    ) => {
                                        *img_file_data = file;
                                    }
                                }
                            } else {
                                todo!();
                            }
                        }

                        read_files
                    });

                    let mut collision = Collision::default();
                    let render_map = RenderMap::new();

                    // load images, external images and do map buffering
                    let mut images_to_load: Vec<(
                        Vec<u8>,
                        u32,
                        u32,
                        GraphicsBackendMemory,
                        usize,
                        usize,
                        usize,
                        usize,
                        GraphicsBackendMemory,
                    )> = Vec::new();
                    let mut images_to_load2: Vec<(
                        Vec<u8>,
                        u32,
                        u32,
                        GraphicsBackendMemory,
                        usize,
                        usize,
                        usize,
                        usize,
                        GraphicsBackendMemory,
                    )> = Vec::new();

                    let mut upload_data = ClientMapBufferUploadData::default();
                    let mut img_tmp: Vec<MapImage> = Vec::new();
                    std::mem::swap(&mut img_tmp, &mut file_wrap.images);

                    let data_file_clone = file_wrap.data_file.clone();

                    // check which images are used in tile & quad layers
                    let mut image_flags: Vec<(bool, bool)> =
                        vec![(Default::default(), Default::default()); img_tmp.len()];

                    for g in 0..file_wrap.num_groups() as usize {
                        let group = file_wrap.get_group(g);

                        for l in 0..group.num_layers as usize {
                            let layer_index = group.start_layer as usize + l;
                            let layer = file_wrap.get_layer(layer_index);
                            match layer {
                                MapLayer::Tile(tile_layer) => {
                                    if tile_layer.0.image != -1
                                        && (tile_layer.0.image as usize) < image_flags.len()
                                    {
                                        image_flags[tile_layer.0.image as usize].0 = true;
                                    }
                                }
                                MapLayer::Quads(quad_layer) => {
                                    if quad_layer.0.image != -1
                                        && (quad_layer.0.image as usize) < image_flags.len()
                                    {
                                        image_flags[quad_layer.0.image as usize].1 = true;
                                    }
                                }
                                _ => {}
                            }
                        }
                    }

                    let generate_3d_data = |index: usize, w: usize, h: usize, img_data: &[u8]| {
                        let in_tile_layer = image_flags[index].0;
                        if in_tile_layer {
                            // first check image dimensions
                            let mut convert_width = w;
                            let mut convert_height = h;
                            let image_color_channels = 4;

                            let mut upload_data = img_data;
                            let mut conv_data: Vec<u8> = Vec::new();

                            if convert_width == 0
                                || (convert_width % 16) != 0
                                || convert_height == 0
                                || (convert_height % 16) != 0
                            {
                                // TODO sys.log("image").msg("3D/2D array texture was resized");
                                let new_width = std::cmp::max(
                                    highest_bit(convert_width as u32) as usize,
                                    16 as usize,
                                );
                                let new_height = std::cmp::max(
                                    highest_bit(convert_height as u32) as usize,
                                    16 as usize,
                                );
                                conv_data = resize(
                                    &runtime_tp,
                                    upload_data,
                                    convert_width,
                                    convert_height,
                                    new_width,
                                    new_height,
                                    image_color_channels,
                                );

                                convert_width = new_width;
                                convert_height = new_height;

                                upload_data = conv_data.as_slice();
                            }

                            let mut tex_3d = graphics_mt.mem_alloc(
                                GraphicsMemoryAllocationType::Texture,
                                image_color_channels as usize
                                    * convert_width as usize
                                    * convert_height as usize,
                            );
                            let mut image_3d_width = 0 as usize;
                            let mut image_3d_height = 0 as usize;
                            if !texture_2d_to_3d(
                                &runtime_tp,
                                upload_data,
                                convert_width,
                                convert_height,
                                image_color_channels,
                                16,
                                16,
                                tex_3d.as_mut_slice(),
                                &mut image_3d_width,
                                &mut image_3d_height,
                            ) {
                                panic!(
                                    "fatal error, could not convert 2d texture to 2d array texture"
                                );
                            }

                            return (image_3d_width, image_3d_height, 256, tex_3d);
                        }
                        return (0, 0, 0, Default::default());
                    };

                    let handle = tokio::runtime::Handle::current();
                    runtime_tp.install(|| {
                        join_all!(
                            || {
                                let g = handle.enter();
                                let mut read_files =
                                    tokio::task::block_in_place(|| handle.block_on(task_read))
                                        .unwrap();
                                drop(g);

                                for (_read_file_path, read_file_info) in read_files.drain() {
                                    match read_file_info {
                                        shared_base::datafile::ReadFile::Image(
                                            image_index,
                                            img_file_data,
                                        ) => {
                                            images_to_load.push((
                                                img_file_data,
                                                0,
                                                0,
                                                Default::default(),
                                                image_index,
                                                0,
                                                0,
                                                0,
                                                Default::default(),
                                            ));
                                        }
                                    }
                                }

                                benchmark!(
                                    do_benchmark,
                                    &sys_time,
                                    "decompressing all external map images",
                                    || {
                                        images_to_load.par_iter_mut().enumerate().for_each(
                                            |(_index, data)| {
                                                let mut img_mem: GraphicsBackendMemory =
                                                    Default::default();
                                                let png_img = load_png_image(&data.0, |size| {
                                                    img_mem = graphics_mt.mem_alloc(
                                                        GraphicsMemoryAllocationType::Texture,
                                                        size,
                                                    );
                                                    img_mem.as_mut_slice()
                                                })
                                                .unwrap();

                                                // generate 3d texture if required
                                                let (
                                                    img_3d_width,
                                                    img_3d_height,
                                                    img_3d_depth,
                                                    img_3d_data,
                                                ) = generate_3d_data(
                                                    data.4,
                                                    png_img.width as usize,
                                                    png_img.height as usize,
                                                    png_img.data,
                                                );

                                                *data = (
                                                    Vec::new(),
                                                    png_img.width,
                                                    png_img.height,
                                                    img_mem,
                                                    data.4,
                                                    img_3d_width,
                                                    img_3d_height,
                                                    img_3d_depth,
                                                    img_3d_data,
                                                );
                                            },
                                        );
                                    },
                                );
                            },
                            || {
                                CDatafileWrapper::read_map_layers(
                                    &file_wrap.data_file,
                                    &mut file_wrap.layers,
                                    data_start,
                                    &sys_time,
                                    &MapFileLayersReadOptions {
                                        do_benchmark: do_benchmark,
                                        ..Default::default()
                                    },
                                );

                                // meanwhile prepare map layers
                                benchmark!(
                                    do_benchmark,
                                    &sys_time,
                                    "initialzing the map layers",
                                    || {
                                        file_wrap.init_layers(&runtime_tp);
                                    },
                                );

                                let game_layer = file_wrap.get_game_layer();
                                let w = game_layer.0.width as u32;
                                let h = game_layer.0.height as u32;

                                let tiles = game_layer.2.as_slice();
                                collision = Collision::new(w, h, tiles);

                                benchmark!(
                                    do_benchmark,
                                    &sys_time,
                                    "preparing the map buffering",
                                    || {
                                        upload_data = ClientMapBuffered::prepare_upload(
                                            &graphics_mt,
                                            &file_wrap,
                                            false,
                                        );
                                    },
                                );
                            },
                            || {
                                benchmark!(
                                    do_benchmark,
                                    &sys_time,
                                    "reading internal map images",
                                    || {
                                        // read all images
                                        let mut ext_imgs = CDatafileWrapper::read_image_data(
                                            &data_file_clone,
                                            &img_tmp,
                                            data_start,
                                            &sys_time,
                                            &MapFileImageReadOptions {
                                                do_benchmark: do_benchmark,
                                            },
                                        );

                                        ext_imgs.drain(..).enumerate().for_each(|(i, img)| {
                                            if let Some(img_data) = img {
                                                // generate 3d texture if required
                                                let (
                                                    img_3d_width,
                                                    img_3d_height,
                                                    img_3d_depth,
                                                    img_3d_data,
                                                ) = generate_3d_data(
                                                    i,
                                                    img_data.0 as usize,
                                                    img_data.1 as usize,
                                                    img_data.2.as_slice(),
                                                );

                                                let mut img_mem = graphics_mt.mem_alloc(
                                                    GraphicsMemoryAllocationType::Texture,
                                                    img_data.0 as usize * img_data.1 as usize * 4,
                                                );
                                                img_mem.copy_from_slice(&img_data.2[..]);

                                                images_to_load2.push((
                                                    Vec::new(),
                                                    img_data.0,
                                                    img_data.1,
                                                    img_mem,
                                                    i,
                                                    img_3d_width,
                                                    img_3d_height,
                                                    img_3d_depth,
                                                    img_3d_data,
                                                ));
                                            }
                                        });

                                        img_tmp.iter_mut().enumerate().for_each(|(index, img)| {});
                                    }
                                );
                            }
                        );
                    });

                    file_wrap.images = img_tmp;

                    for (_, w, h, data, img_index, w_3d, h_3d, d_3d, data_3d) in
                        images_to_load.drain(..).chain(images_to_load2.drain(..))
                    {
                        let img = &mut file_wrap.images[img_index];
                        img.img_data = data;
                        img.item_data.width = w as i32;
                        img.item_data.height = h as i32;

                        // set where the image is used
                        let in_tile_layer = image_flags[img_index].0;
                        let in_tile_quad = image_flags[img_index].1;
                        img.img_used = in_tile_quad;
                        img.img_3d_used = in_tile_layer;

                        // set 3d data
                        img.img_3d_width = w_3d;
                        img.img_3d_height = h_3d;
                        img.img_3d_depth = d_3d;
                        img.img_3d_data = data_3d;
                    }

                    Ok(ClientMapFileProcessed {
                        raw: file_wrap,
                        render: render_map,
                        collision: collision,
                        upload_data: upload_data,
                    })
                })
                .await
                .unwrap()
            }),
        }
    }
}

pub enum ClientMap {
    Map(ClientMapFile),
    UploadingImagesAndMapBuffer(ClientMapLoadingFile),
    None,
}

impl ClientMap {
    pub fn unwrap_data_and_game_mut(&mut self) -> (&ClientMapFileData, &mut GameStateWasmManager) {
        self.try_get_data_and_game_mut()
            .ok_or("map file was not loaded yet")
            .unwrap()
    }

    pub fn unwrap(&self) -> &ClientMapFile {
        self.try_get().ok_or("map file was not loaded yet").unwrap()
    }

    pub fn try_get(&self) -> Option<&ClientMapFile> {
        if let Self::Map(map_file) = self {
            Some(map_file)
        } else {
            None
        }
    }

    pub fn try_get_data_and_game_mut(
        &mut self,
    ) -> Option<(&ClientMapFileData, &mut GameStateWasmManager)> {
        if let Self::Map(map_file) = self {
            Some((&map_file.data, &mut map_file.game))
        } else {
            None
        }
    }

    pub fn is_fully_loaded(&self) -> bool {
        if let Self::Map(_map_file) = self {
            return true;
        }
        false
    }

    pub fn continue_loading(
        &mut self,
        thread_pool: &Arc<rayon::ThreadPool>,
        io_batcher: &Arc<std::sync::Mutex<TokIOBatcher>>,
        fs: &Arc<FileSystem>,
        graphics: &mut Graphics,
        config: &Config,
        sys: &Arc<impl SystemTimeInterface + Send + Sync + 'static>,
    ) -> Option<&ClientMapFile> {
        let do_benchmark = config.dbg_bench;
        match self {
            Self::UploadingImagesAndMapBuffer(map_upload) => {
                if map_upload.task.is_finished() {
                    io_batcher
                        .lock()
                        .unwrap()
                        .wait_finished_and_drop(&mut map_upload.task);
                    let mut map_file = map_upload.task.get_storage().unwrap();
                    let runtime_tp = thread_pool.clone();
                    let mut images: Vec<ClientMapImage> = Default::default();

                    benchmark!(
                        do_benchmark,
                        &sys,
                        "creating the image graphics cmds",
                        || {
                            map_file.raw.images.drain(..).for_each(|img| {
                                let img_data = img.img_data;
                                let img_flag = TexFlags::empty();
                                let mut texture_id = None;
                                let mut texture_id_3d = None;
                                if img.img_3d_used {
                                    texture_id_3d = Some(
                                        graphics
                                            .load_texture_3d(
                                                img.img_3d_width,
                                                img.img_3d_height,
                                                img.img_3d_depth,
                                                ImageFormat::Rgba as i32,
                                                img.img_3d_data,
                                                ImageFormat::Rgba as i32,
                                                img_flag,
                                                &img.img_name,
                                            )
                                            .unwrap(),
                                    );
                                }
                                if img.img_used {
                                    texture_id = Some(
                                        graphics
                                            .load_texture(
                                                img.item_data.width as usize,
                                                img.item_data.height as usize,
                                                ImageFormat::Rgba as i32,
                                                img_data,
                                                ImageFormat::Rgba as i32,
                                                img_flag,
                                                &img.img_name,
                                            )
                                            .unwrap(),
                                    );
                                }
                                images.push(ClientMapImage {
                                    texture_index: texture_id,
                                    texture_index_3d: texture_id_3d,
                                });
                            });
                        },
                    );

                    let mut map_buffered = ClientMapBuffered::new();
                    benchmark!(
                        do_benchmark,
                        &sys,
                        "creating the map buffers graphics cmds",
                        || {
                            map_buffered.upload_map(graphics, map_file.upload_data);
                        },
                    );

                    let game = GameStateWasmManager::new(
                        &GameStateCreatePipe {
                            game_layer: map_file.raw.get_game_layer(),
                            cur_time: sys.time_get_nanoseconds(),
                        },
                        &Default::default(),
                        fs,
                        io_batcher,
                    );
                    *self = Self::Map(ClientMapFile {
                        data: ClientMapFileData {
                            raw: map_file.raw,
                            images: images,
                            render: map_file.render,
                            collision: map_file.collision,
                            buffered_map: map_buffered,
                        },
                        game,
                    });

                    return Some(self.unwrap());
                } else {
                    None
                }
            }
            Self::Map(map) => Some(map),
            Self::None => None,
        }
    }

    pub fn destroy(
        &mut self,
        thread_pool: &Arc<rayon::ThreadPool>,
        io_batcher: &Arc<std::sync::Mutex<TokIOBatcher>>,
        fs: &Arc<FileSystem>,
        graphics: &mut Graphics,
        config: &Config,
        sys: &Arc<impl SystemTimeInterface + Send + Sync + 'static>,
    ) {
        match self {
            ClientMap::Map(map) => {
                let mut buffered_map = ClientMapBuffered::new();
                std::mem::swap(&mut buffered_map, &mut map.data.buffered_map);
                buffered_map.map_destroy(graphics);
                for img in &mut map.data.images {
                    let mut img_swap = None;
                    std::mem::swap(&mut img.texture_index, &mut img_swap);
                    if let Some(tex_id) = img_swap {
                        graphics.unload_texture(tex_id);
                    }
                    let mut img_swap = None;
                    std::mem::swap(&mut img.texture_index_3d, &mut img_swap);
                    if let Some(tex_id) = img_swap {
                        graphics.unload_texture(tex_id);
                    }
                }
            }
            ClientMap::UploadingImagesAndMapBuffer(map) => {
                io_batcher
                    .lock()
                    .unwrap()
                    .wait_finished_and_drop(&mut map.task);
                self.continue_loading(thread_pool, io_batcher, fs, graphics, config, sys);
                self.destroy(thread_pool, io_batcher, fs, graphics, config, sys);
            }
            ClientMap::None => {
                // nothing to do
            }
        }
    }
}
