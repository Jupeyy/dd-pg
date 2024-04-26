use std::{
    collections::{HashMap, HashSet},
    path::Path,
    sync::Arc,
};

use anyhow::anyhow;
use base::{
    benchmark::Benchmark,
    hash::{fmt_hash, Hash},
    join_all,
};
use base_io::{io::IO, io_batcher::IOBatcherTask};
use client_render_base::map::{
    map::RenderMap,
    map_buffered::{ClientMapBufferUploadData, ClientMapBuffered},
    map_image::{
        ClientMapImageLoading, ClientMapImagesLoading, ClientMapSoundLoading,
        ClientMapSoundsLoading,
    },
};
use config::config::{ConfigDebug, ConfigEngine};
use graphics::{
    graphics::graphics::Graphics,
    handles::{
        backend::backend::GraphicsBackendHandle,
        buffer_object::buffer_object::GraphicsBufferObjectHandle,
        canvas::canvas::GraphicsCanvasHandle,
        stream::stream::GraphicsStreamHandle,
        texture::texture::{GraphicsTextureHandle, TextureContainer, TextureContainer2dArray},
    },
    image::{highest_bit, resize, texture_2d_to_3d},
};
use graphics_types::{
    commands::{TexFlags, TexFormat},
    types::{GraphicsMemoryAllocationType, ImageFormat},
};
use image::png::load_png_image;
use map::map::Map;
use math::math::vector::vec2;
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use shared_game::collision::collision::Collision;
use sound::{scene_handle::SoundSceneHandle, sound::SoundManager};
use url::Url;

pub struct ClientMapFileData {
    pub collision: Collision,
    pub buffered_map: ClientMapBuffered,
}

pub struct ClientMapRenderAndFile {
    pub data: ClientMapFileData,
    pub render: RenderMap,
}

pub struct ClientMapFileProcessed {
    pub upload_data: ClientMapBufferUploadData,
    pub collision: Collision,
    pub images: ClientMapImagesLoading,
    pub sounds: ClientMapSoundsLoading,
}

pub struct RenderMapLoading {
    pub task: IOBatcherTask<ClientMapFileProcessed>,
    pub backend_handle: GraphicsBackendHandle,
    pub buffer_object_handle: GraphicsBufferObjectHandle,
    pub texture_handle: GraphicsTextureHandle,
    pub canvas_handle: GraphicsCanvasHandle,
    pub stream_handle: GraphicsStreamHandle,

    pub sound_scene_handle: SoundSceneHandle,
}

impl RenderMapLoading {
    pub fn new(
        thread_pool: Arc<rayon::ThreadPool>,
        file: Vec<u8>,
        resource_download_server: Option<Url>,
        io: IO,
        sound: &SoundManager,
        graphics: &Graphics,
        config: &ConfigEngine,
    ) -> Self {
        let file_system = io.fs.clone();
        let http = io.http.clone();
        let do_benchmark = config.dbg.bench;
        let runtime_tp = thread_pool;
        let graphics_mt = graphics.get_graphics_mt();
        let sound_mt = sound.get_sound_mt();
        Self {
            task: io.io_batcher.spawn(async move {
                let benchmark = Benchmark::new(do_benchmark);
                // open the map file
                let (resources, resources_bytes_read) = Map::read_resources_and_header(&file)?;
                benchmark.bench("opening the full map file");

                // read content files
                let mut file_map: HashSet<Hash> = Default::default();
                #[derive(Debug, PartialEq, Clone, Copy)]
                enum ReadFileTy {
                    Image,
                    Sound,
                }
                let file_futures = resources
                    .images
                    .iter()
                    .map(|i| (i, ReadFileTy::Image))
                    .chain(
                        resources
                            .image_arrays
                            .iter()
                            .map(|i| (i, ReadFileTy::Image)),
                    )
                    .chain(resources.sounds.iter().map(|s| (s, ReadFileTy::Sound)))
                    .filter(|(i, _)| {
                        if file_map.contains(&i.blake3_hash) {
                            false
                        } else {
                            file_map.insert(i.blake3_hash);
                            true
                        }
                    })
                    .map(|(res, ty)| {
                        let read_file_path = format!(
                            "map/resources/{}/{}_{}.{}",
                            if ty == ReadFileTy::Image {
                                "images"
                            } else {
                                "sounds"
                            },
                            res.name,
                            fmt_hash(&res.blake3_hash),
                            res.ty
                        );
                        let hash = res.blake3_hash;
                        let fs = file_system.clone();
                        let http = http.clone();
                        let resource_download_server = resource_download_server.clone();
                        async move {
                            let file = fs.open_file(Path::new(&read_file_path)).await;

                            let file = match file {
                                Ok(file) => Ok(file),
                                Err(err) => {
                                    async move {
                                        // try to download file
                                        if let Some(resource_download_server) =
                                            resource_download_server
                                                .map(|url| url.join(&read_file_path).ok())
                                                .flatten()
                                        {
                                            let file = http
                                                .download_binary(resource_download_server, &hash)
                                                .await
                                                .map_err(|err| {
                                                    anyhow!("failed to download map: {err}")
                                                })?
                                                .to_vec();
                                            // TODO: ensure that downloaded resource is an working/valid image/sound file
                                            let file_path: &Path = read_file_path.as_ref();
                                            if let Some(dir) = file_path.parent() {
                                                fs.create_dir(dir).await?;
                                            }
                                            fs.write_file(read_file_path.as_ref(), file.clone())
                                                .await?;
                                            anyhow::Ok(file)
                                        } else {
                                            Err(anyhow!(err))
                                        }
                                    }
                                    .await
                                }
                            }
                            .map_err(|err| anyhow!(err));

                            (hash, file, ty)
                        }
                    });
                let task_read = futures::future::join_all(file_futures);

                // poll once with a small hack
                let task_read = futures::future::maybe_done(task_read);
                futures::pin_mut!(task_read);
                futures::future::FutureExt::now_or_never(&mut task_read);

                task_read.as_mut().await;
                let files = task_read.as_mut().take_output().unwrap();
                let mut img_files: HashMap<Hash, Vec<u8>> = Default::default();
                let mut sound_files: HashMap<Hash, Vec<u8>> = Default::default();
                for (file_hash, file, ty) in files {
                    let file = file?;
                    match ty {
                        ReadFileTy::Image => {
                            img_files.insert(file_hash, file);
                        }
                        ReadFileTy::Sound => {
                            sound_files.insert(file_hash, file);
                        }
                    }
                }

                let resources_clone = resources.clone();

                let generate_3d_data = |w: usize, h: usize, img_data: &[u8]| {
                    // first check image dimensions
                    let mut convert_width = w;
                    let mut convert_height = h;
                    let image_color_channels = 4;

                    let mut upload_data = img_data;
                    let conv_data: Vec<u8>;

                    if convert_width == 0
                        || (convert_width % 16) != 0
                        || convert_height == 0
                        || (convert_height % 16) != 0
                    {
                        // TODO sys.log("image").msg("3D/2D array texture was resized");
                        let new_width =
                            std::cmp::max(highest_bit(convert_width as u32) as usize, 16);
                        let new_height =
                            std::cmp::max(highest_bit(convert_height as u32) as usize, 16);
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

                    let mut tex_3d = graphics_mt.mem_alloc(GraphicsMemoryAllocationType::Texture {
                        width: convert_width / 16,
                        height: convert_height / 16,
                        depth: 256,
                        is_3d_tex: true,
                        flags: TexFlags::empty(),
                    });
                    let mut image_3d_width = 0;
                    let mut image_3d_height = 0;
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
                        panic!("fatal error, could not convert 2d texture to 2d array texture");
                    }

                    if graphics_mt.try_flush_mem(&mut tex_3d, false).is_err() {
                        // TODO: ignore?
                    }

                    (image_3d_width, image_3d_height, 256, tex_3d)
                };
                // load images, external images and do map buffering
                let (images_loading, sounds_loading, map_prepare) = runtime_tp.install(|| {
                    join_all!(
                        || {
                            let img_files = img_files
                                .into_par_iter()
                                .map(|(hash, file)| {
                                    let mut img_data: Vec<u8> = Default::default();
                                    let img = load_png_image(
                                        &file,
                                        |width, height, color_channel_count| {
                                            img_data.resize(
                                                width * height * color_channel_count,
                                                Default::default(),
                                            );
                                            &mut img_data
                                        },
                                    )?;
                                    anyhow::Ok((hash, (img.data.to_vec(), img.width, img.height)))
                                })
                                .collect::<anyhow::Result<HashMap<Hash, (Vec<u8>, u32, u32)>>>()?;

                            let mut images_loading = ClientMapImagesLoading::default();
                            images_loading.images = resources_clone
                                .images
                                .into_par_iter()
                                .map(|img| {
                                    let (img_data, width, height) = img_files
                                        .get(&img.blake3_hash)
                                        .ok_or(anyhow!("img with that name not found"))?;
                                    let mut loading_img = ClientMapImageLoading {
                                        mem: graphics_mt.mem_alloc(
                                            GraphicsMemoryAllocationType::Texture {
                                                width: *width as usize,
                                                height: *height as usize,
                                                depth: 1,
                                                is_3d_tex: false,
                                                flags: TexFlags::empty(),
                                            },
                                        ),
                                        width: *width,
                                        height: *height,
                                        depth: 1,
                                        name: img.name,
                                    };
                                    loading_img.mem.as_mut_slice().copy_from_slice(img_data);
                                    if graphics_mt
                                        .try_flush_mem(&mut loading_img.mem, false)
                                        .is_err()
                                    {
                                        // TODO: handle/log ?
                                    }
                                    anyhow::Ok(loading_img)
                                })
                                .collect::<anyhow::Result<Vec<ClientMapImageLoading>>>()?;
                            images_loading.images_2d_array = resources_clone
                                .image_arrays
                                .into_par_iter()
                                .map(|img| {
                                    let (img_data, width, height) = img_files
                                        .get(&img.blake3_hash)
                                        .ok_or(anyhow!("img with that name not found"))?;
                                    let (width, height, depth, mem) = generate_3d_data(
                                        *width as usize,
                                        *height as usize,
                                        img_data,
                                    );
                                    anyhow::Ok(ClientMapImageLoading {
                                        mem,
                                        width: width as u32,
                                        height: height as u32,
                                        depth: depth as u32,
                                        name: img.name,
                                    })
                                })
                                .collect::<anyhow::Result<Vec<ClientMapImageLoading>>>()?;

                            benchmark.bench_multi("decompressing all external map images");
                            anyhow::Ok(images_loading)
                        },
                        || {
                            let sounds_loading = sound_files
                                .into_par_iter()
                                .map(|(_, file)| {
                                    let mut mem = sound_mt.mem_alloc(file.len());
                                    mem.as_mut_slice().copy_from_slice(&file);
                                    let _ = sound_mt.try_flush_mem(&mut mem); // ignore error on purpose
                                    anyhow::Ok(ClientMapSoundLoading { mem })
                                })
                                .collect::<anyhow::Result<Vec<ClientMapSoundLoading>>>()?;

                            benchmark.bench_multi("decompressing all internal sounds");
                            anyhow::Ok(sounds_loading)
                        },
                        || {
                            let map = Map::read_with_resources(
                                resources,
                                &file[resources_bytes_read..],
                                &runtime_tp,
                            )?;

                            benchmark.bench_multi("initialzing the map layers");

                            let physics_group = &map.groups.physics;
                            let collision = Collision::new(
                                physics_group.attr.width.get() as u32,
                                physics_group.attr.height.get() as u32,
                                physics_group.get_game_layer_tiles(),
                                None,
                            );

                            let upload_data = ClientMapBuffered::prepare_upload(&graphics_mt, map);
                            benchmark.bench_multi("preparing the map buffering");

                            anyhow::Ok((collision, upload_data))
                        }
                    )
                });

                benchmark.bench("loading the full map (excluding opening it)");

                let (collision, upload_data) = map_prepare?;
                Ok(ClientMapFileProcessed {
                    collision,
                    upload_data,
                    images: images_loading?,
                    sounds: sounds_loading?,
                })
            }),
            backend_handle: graphics.backend_handle.clone(),
            buffer_object_handle: graphics.buffer_object_handle.clone(),
            texture_handle: graphics.texture_handle.clone(),
            canvas_handle: graphics.canvas_handle.clone(),
            stream_handle: graphics.stream_handle.clone(),

            sound_scene_handle: sound.scene_handle.clone(),
        }
    }
}

pub enum ClientMapRender {
    UploadingBuffersAndTextures(RenderMapLoading),
    Map(ClientMapRenderAndFile),
    None,
}

impl ClientMapRender {
    pub fn new(loading: RenderMapLoading) -> Self {
        Self::UploadingBuffersAndTextures(loading)
    }

    pub fn try_get(&self) -> Option<&ClientMapRenderAndFile> {
        if let Self::Map(map_file) = self {
            Some(map_file)
        } else {
            None
        }
    }

    pub fn continue_loading(&mut self, config: &ConfigDebug) -> Option<&ClientMapRenderAndFile> {
        let do_benchmark = config.bench;
        let mut self_helper = Self::None;
        std::mem::swap(&mut self_helper, self);
        match self_helper {
            Self::UploadingBuffersAndTextures(map_upload) => {
                if map_upload.task.is_finished() {
                    // the task might be cleared by a higher function call, so make sure it still exists
                    let map_file = map_upload.task.get_storage().unwrap();

                    let benchmark = Benchmark::new(do_benchmark);

                    let images = map_file
                        .images
                        .images
                        .into_iter()
                        .map(|img| {
                            map_upload.texture_handle.load_texture(
                                img.width as usize,
                                img.height as usize,
                                ImageFormat::Rgba,
                                img.mem,
                                TexFormat::RGBA,
                                TexFlags::empty(),
                                &img.name,
                            )
                        })
                        .collect::<anyhow::Result<Vec<TextureContainer>>>()
                        .unwrap();
                    let images_2d_array = map_file
                        .images
                        .images_2d_array
                        .into_iter()
                        .map(|img| {
                            map_upload.texture_handle.load_texture_3d(
                                img.width as usize,
                                img.height as usize,
                                img.depth as usize,
                                ImageFormat::Rgba,
                                img.mem,
                                TexFormat::RGBA,
                                TexFlags::empty(),
                                &img.name,
                            )
                        })
                        .collect::<anyhow::Result<Vec<TextureContainer2dArray>>>()
                        .unwrap();

                    // sound scene
                    let scene = map_upload.sound_scene_handle.create();
                    let listener = scene.sound_listener_handle.create(vec2::default());
                    let sound_objects: Vec<_> = map_file
                        .sounds
                        .into_iter()
                        .map(|sound| scene.sound_object_handle.create(sound.mem))
                        .collect();

                    benchmark.bench("creating the image graphics cmds");

                    let map_buffered = ClientMapBuffered::new(
                        &map_upload.backend_handle,
                        &map_upload.buffer_object_handle,
                        map_file.upload_data,
                        images,
                        images_2d_array,
                        scene,
                        listener,
                        sound_objects,
                    );

                    benchmark.bench("creating the map buffers graphics cmds");

                    *self = Self::Map(ClientMapRenderAndFile {
                        data: ClientMapFileData {
                            collision: map_file.collision,
                            buffered_map: map_buffered,
                        },
                        render: RenderMap::new(
                            &map_upload.backend_handle,
                            &map_upload.canvas_handle,
                            &map_upload.stream_handle,
                        ),
                    });
                } else {
                    *self = Self::UploadingBuffersAndTextures(map_upload)
                }
            }
            Self::Map(map) => *self = Self::Map(map),
            Self::None => {}
        }
        self.try_get()
    }
}
