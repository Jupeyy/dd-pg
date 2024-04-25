use std::{
    collections::{HashMap, HashSet},
    path::Path,
    sync::Arc,
    time::Duration,
};

use anyhow::anyhow;
use base::{
    hash::{fmt_hash, Hash},
    join_all,
    system::{System, SystemTimeInterface},
};
use base_io::{io::IO, io_batcher::IOBatcherTask};
use client_containers_new::entities::{EntitiesContainer, ENTITIES_CONTAINER_PATH};
use client_render_base::map::{
    map::{ForcedTexture, RenderMap},
    map_buffered::{
        ClientMapBufferPhysicsTileLayer, ClientMapBufferQuadLayer, ClientMapBufferTileLayer,
        SoundLayerSounds,
    },
    render_pipe::Camera,
};
use config::config::ConfigEngine;
use game_config::config::ConfigMap;
use graphics::{
    graphics::graphics::Graphics,
    graphics_mt::GraphicsMultiThreaded,
    handles::{
        backend::backend::GraphicsBackendHandle,
        buffer_object::buffer_object::GraphicsBufferObjectHandle,
        canvas::canvas::GraphicsCanvasHandle,
        stream::stream::GraphicsStreamHandle,
        texture::texture::{GraphicsTextureHandle, TextureContainer, TextureContainer2dArray},
    },
    image::texture_2d_to_3d,
};
use graphics_types::{
    commands::{TexFlags, TexFormat},
    types::{GraphicsMemoryAllocationType, ImageFormat},
};
use hashlink::LinkedHashMap;
use hiarc::HiarcTrait;
use image::png::load_png_image;
use map::{
    map::{
        animations::{AnimBase, AnimPointCurveType, AnimPointPos},
        config::Config,
        groups::{
            layers::{
                design::{MapLayer, MapLayerQuad, MapLayerSound, MapLayerTile},
                physics::{MapLayerPhysics, MapLayerTilePhysicsBase},
                tiles::{MapTileLayerPhysicsTilesRef, TileBase},
            },
            MapGroup, MapGroupAttr, MapGroupPhysicsAttr,
        },
        metadata::Metadata,
        Map,
    },
    skeleton::{
        animations::{AnimBaseSkeleton, AnimationsSkeleton},
        groups::layers::{
            design::MapLayerSkeleton,
            physics::{MapLayerArbitraryPhysicsSkeleton, MapLayerTilePhysicsBaseSkeleton},
        },
    },
    types::NonZeroU16MinusOne,
};
use map_convert_lib::{legacy_to_new::legacy_to_new, new_to_legacy::new_to_legacy};
use math::math::vector::vec2;
use network::network::network::{NetworkClientCertCheckMode, NetworkServerCertMode};
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use serde::de::DeserializeOwned;
use sound::{scene_handle::SoundSceneHandle, sound::SoundManager, sound_mt::SoundMultiThreaded};
use ui_base::font_data::UiFontData;

use crate::{
    client::EditorClient,
    editor_ui::{EditorUiRender, EditorUiRenderPipe},
    event::{EditorEvent, EditorEventOverwriteMap},
    map::{
        EditorAnimations, EditorAnimationsProps, EditorArbitraryLayerProps, EditorColorAnimation,
        EditorCommonGroupOrLayerAttr, EditorCommonLayerOrGroupAttrInterface, EditorConfig,
        EditorGroup, EditorGroupPhysics, EditorGroupProps, EditorGroups, EditorGroupsProps,
        EditorImage, EditorImage2dArray, EditorLayer, EditorLayerArbitrary, EditorLayerQuad,
        EditorLayerSound, EditorLayerTile, EditorLayerUnionRef, EditorMap,
        EditorMapGroupsInterface, EditorMapInterface, EditorMapProps, EditorMetadata,
        EditorPhysicsGroupProps, EditorPhysicsLayer, EditorPhysicsLayerProps, EditorPosAnimation,
        EditorQuadLayerProps, EditorQuadLayerPropsPropsSelection, EditorResource, EditorResources,
        EditorSound, EditorSoundAnimation, EditorSoundLayerProps, EditorTileLayerProps,
        EditorTileLayerPropsSelection, ResourceSelection,
    },
    map_tools::{
        finish_design_quad_layer_buffer, finish_design_tile_layer_buffer,
        finish_physics_layer_buffer, upload_design_quad_layer_buffer,
        upload_design_tile_layer_buffer, upload_physics_layer_buffer,
    },
    notifications::EditorNotifications,
    server::EditorServer,
    tab::EditorTab,
    tools::{
        quad_layer::{brush::QuadBrush, selection::QuadSelection},
        sound_layer::brush::SoundBrush,
        tile_layer::{
            auto_mapper::TileLayerAutoMapper, brush::TileBrush, selection::TileSelection,
        },
        tool::{
            ActiveTool, ActiveToolQuads, ActiveToolSounds, ActiveToolTiles, ToolQuadLayer,
            ToolSoundLayer, ToolTileLayer, Tools,
        },
    },
    ui::user_data::EditorUiEvent,
    utils::ui_pos_to_world_pos,
};

/// this is basically the editor client
pub struct Editor {
    tabs: LinkedHashMap<String, EditorTab>,
    active_tab: String,
    sys: System,

    ui: EditorUiRender,
    // events triggered by ui
    events: Vec<EditorEvent>,
    ui_events: Vec<EditorUiEvent>,

    // editor tool
    tools: Tools,
    auto_mapper: TileLayerAutoMapper,

    middle_down_pointer_pos: Option<egui::Pos2>,
    current_pointer_pos: egui::Pos2,
    current_scroll_delta: egui::Vec2,
    latest_pointer: egui::PointerState,
    latest_keys_down: HashSet<egui::Key>,
    latest_modifiers: egui::Modifiers,
    latest_unused_rect: egui::Rect,
    last_time: Duration,

    // notifications
    notifications: EditorNotifications,

    // graphics
    graphics_mt: GraphicsMultiThreaded,
    buffer_object_handle: GraphicsBufferObjectHandle,
    backend_handle: GraphicsBackendHandle,
    texture_handle: GraphicsTextureHandle,
    canvas_handle: GraphicsCanvasHandle,
    stream_handle: GraphicsStreamHandle,

    // sound
    sound_mt: SoundMultiThreaded,
    scene_handle: SoundSceneHandle,

    entities_container: EntitiesContainer,
    fake_texture_array: TextureContainer2dArray,
    fake_texture: TextureContainer,

    // misc
    io: IO,
    thread_pool: Arc<rayon::ThreadPool>,
}

impl Editor {
    pub fn new(
        sound: &SoundManager,
        graphics: &Graphics,
        io: &IO,
        tp: &Arc<rayon::ThreadPool>,
        font_data: &Arc<UiFontData>,
    ) -> Self {
        let sys = System::new();
        let default_entities =
            EntitiesContainer::load_default(io, ENTITIES_CONTAINER_PATH.as_ref());
        let scene = sound.scene_handle.create();
        let entities_container = EntitiesContainer::new(
            io.clone(),
            tp.clone(),
            default_entities,
            &sys.log,
            None,
            None,
            "entities-container",
            graphics,
            sound,
            &scene,
            ENTITIES_CONTAINER_PATH.as_ref(),
        );

        // fake texture array texture for non textured layers
        let mut mem = graphics
            .get_graphics_mt()
            .mem_alloc(GraphicsMemoryAllocationType::Texture {
                width: 1,
                height: 1,
                depth: 256,
                is_3d_tex: true,
                flags: TexFlags::empty(),
            });
        mem.as_mut_slice().iter_mut().for_each(|byte| *byte = 255);
        // clear first tile, must stay empty
        mem.as_mut_slice()[0..4].copy_from_slice(&[0, 0, 0, 0]);

        let fake_texture_array = graphics
            .texture_handle
            .load_texture_3d(
                1,
                1,
                256,
                ImageFormat::Rgba,
                mem,
                TexFormat::RGBA,
                TexFlags::empty(),
                "fake-editor-texture",
            )
            .unwrap();

        // fake texture texture for non textured quads
        let mut mem = graphics
            .get_graphics_mt()
            .mem_alloc(GraphicsMemoryAllocationType::Texture {
                width: 1,
                height: 1,
                depth: 1,
                is_3d_tex: false,
                flags: TexFlags::empty(),
            });
        mem.as_mut_slice().iter_mut().for_each(|byte| *byte = 255);

        let fake_texture = graphics
            .texture_handle
            .load_texture(
                1,
                1,
                ImageFormat::Rgba,
                mem,
                TexFormat::RGBA,
                TexFlags::empty(),
                "fake-editor-texture",
            )
            .unwrap();

        let last_time = sys.time_get_nanoseconds();

        let graphics_mt = graphics.get_graphics_mt();
        let mut res = Self {
            tabs: Default::default(),
            active_tab: "".into(),
            sys,

            ui: EditorUiRender::new(graphics, font_data),
            events: Default::default(),
            ui_events: Default::default(),

            tools: Tools {
                tiles: ToolTileLayer {
                    brush: TileBrush::new(
                        &graphics_mt,
                        &graphics.buffer_object_handle,
                        &graphics.backend_handle,
                    ),
                    selection: TileSelection::new(),
                },
                quads: ToolQuadLayer {
                    brush: QuadBrush::new(),
                    selection: QuadSelection::new(),
                },
                sounds: ToolSoundLayer {
                    brush: SoundBrush::new(),
                },
                active_tool: ActiveTool::Tiles(ActiveToolTiles::Brush),
            },
            auto_mapper: TileLayerAutoMapper::new(io.clone().into(), tp.clone()),
            middle_down_pointer_pos: None,
            current_scroll_delta: Default::default(),
            current_pointer_pos: Default::default(),
            latest_pointer: Default::default(),
            latest_keys_down: Default::default(),
            latest_modifiers: Default::default(),
            latest_unused_rect: egui::Rect::from_min_size(
                egui::Pos2 { x: 0.0, y: 0.0 },
                egui::Vec2 { x: 100.0, y: 100.0 },
            ),
            last_time,

            notifications: Default::default(),

            graphics_mt,
            buffer_object_handle: graphics.buffer_object_handle.clone(),
            backend_handle: graphics.backend_handle.clone(),
            texture_handle: graphics.texture_handle.clone(),
            canvas_handle: graphics.canvas_handle.clone(),
            stream_handle: graphics.stream_handle.clone(),

            scene_handle: sound.scene_handle.clone(),
            sound_mt: sound.get_sound_mt(),

            entities_container,
            fake_texture_array,
            fake_texture,

            io: io.clone(),
            thread_pool: tp.clone(),
        };
        res.load_map("map/maps/ctf1.twmap".as_ref());
        res
    }

    pub fn new_map(
        &mut self,
        name: &str,
        server_cert_hash: Option<Hash>,
        server_addr: Option<String>,
        server_password: Option<String>,
    ) {
        let server = server_cert_hash
            .is_none()
            .then(|| EditorServer::new(&self.sys, None, None, "".into()));
        let client = EditorClient::new(
            &self.sys,
            &if let Some(server_addr) = &server_addr {
                server_addr.clone()
            } else {
                format!(
                    "0.0.0.0:{}",
                    server
                        .as_ref()
                        .map(|server| server.port)
                        .unwrap_or_default()
                )
            },
            match &server {
                Some(server) => NetworkClientCertCheckMode::CheckByCert { cert: &server.cert },
                None => match &server_cert_hash {
                    Some(hash) => NetworkClientCertCheckMode::CheckByCertHash { hash: &hash },
                    None => panic!("this should not happen: server and server cert hash are None"),
                },
            },
            self.notifications.clone(),
            server_password.unwrap_or_default(),
            server_addr.is_none(),
        );

        let physics_group_attr = MapGroupPhysicsAttr {
            width: NonZeroU16MinusOne::new(50).unwrap(),
            height: NonZeroU16MinusOne::new(50).unwrap(),
        };
        let game_layer = MapLayerTilePhysicsBase {
            tiles: vec![TileBase::default(); 50 * 50],
        };
        let visuals = {
            let buffer = self.thread_pool.install(|| {
                upload_physics_layer_buffer(
                    &self.graphics_mt,
                    physics_group_attr.width,
                    physics_group_attr.height,
                    MapTileLayerPhysicsTilesRef::Game(&game_layer.tiles),
                )
            });
            finish_physics_layer_buffer(&self.buffer_object_handle, &self.backend_handle, buffer)
        };

        let scene = self.scene_handle.create();
        let global_sound_listener = scene.sound_listener_handle.create(Default::default());

        self.tabs.insert(
            name.into(),
            EditorTab {
                map: EditorMap {
                    user: EditorMapProps {
                        options: Default::default(),
                        ui_values: Default::default(),
                        sound_scene: scene,
                        global_sound_listener,
                        time: Duration::ZERO,
                        time_scale: 0,
                    },
                    resources: EditorResources {
                        images: Default::default(),
                        image_arrays: Default::default(),
                        sounds: Default::default(),
                        user: (),
                    },
                    animations: EditorAnimations {
                        pos: Default::default(),
                        color: Default::default(),
                        sound: Default::default(),
                        user: EditorAnimationsProps::default(),
                    },
                    groups: EditorGroups {
                        physics: EditorGroupPhysics {
                            attr: physics_group_attr,
                            layers: vec![EditorPhysicsLayer::Game(
                                MapLayerTilePhysicsBaseSkeleton {
                                    def: game_layer,
                                    user: EditorPhysicsLayerProps {
                                        visuals,
                                        attr: EditorCommonGroupOrLayerAttr::default(),
                                        selected: Default::default(),
                                    },
                                },
                            )],
                            user: EditorPhysicsGroupProps::default(),
                        },
                        background: Vec::new(),
                        foreground: Vec::new(),
                        user: EditorGroupsProps {
                            pos: Default::default(),
                            zoom: 1.0,
                        },
                    },
                    config: EditorConfig {
                        def: Config {
                            commands: Default::default(),
                        },
                        user: (),
                    },
                    meta: EditorMetadata {
                        def: Metadata {
                            authors: Default::default(),
                            licenses: Default::default(),
                            version: Default::default(),
                            credits: Default::default(),
                            memo: Default::default(),
                        },
                        user: (),
                    },
                },
                map_render: RenderMap::new(
                    &self.backend_handle,
                    &self.canvas_handle,
                    &self.stream_handle,
                ),
                server,
                client,
            },
        );
        self.active_tab = name.into();
    }

    fn map_to_editor_map_impl(
        graphics_mt: GraphicsMultiThreaded,
        sound_mt: SoundMultiThreaded,
        tp: &Arc<rayon::ThreadPool>,
        scene_handle: &SoundSceneHandle,
        backend_handle: &GraphicsBackendHandle,
        buffer_object_handle: &GraphicsBufferObjectHandle,
        texture_handle: &GraphicsTextureHandle,
        map: Map,
        resources: HashMap<Hash, Vec<u8>>,
    ) -> EditorMap {
        // load images into VRAM
        let (image_mems, image_array_mems, sound_mems): (Vec<_>, Vec<_>, Vec<_>) =
            tp.install(|| {
                join_all!(
                    || {
                        map.resources
                            .images
                            .into_par_iter()
                            .map(|i| {
                                let file = resources.get(&i.blake3_hash).unwrap();

                                let mut mem = None;
                                let img = load_png_image(file, |width, height, _| {
                                    mem = Some(graphics_mt.mem_alloc(
                                        GraphicsMemoryAllocationType::Texture {
                                            width,
                                            height,
                                            depth: 1,
                                            is_3d_tex: false,
                                            flags: TexFlags::empty(),
                                        },
                                    ));
                                    mem.as_mut().unwrap().as_mut_slice()
                                })
                                .unwrap();

                                (i, img.to_persistent(), mem.unwrap(), file.clone())
                            })
                            .collect()
                    },
                    || {
                        map.resources
                            .image_arrays
                            .into_par_iter()
                            .map(|i| {
                                let file = resources.get(&i.blake3_hash).unwrap();

                                let mut png = Vec::new();
                                let img =
                                    load_png_image(file, |width, height, color_chanel_count| {
                                        png.resize(
                                            width * height * color_chanel_count,
                                            Default::default(),
                                        );
                                        png.as_mut_slice()
                                    })
                                    .unwrap();

                                let mut mem =
                                    graphics_mt.mem_alloc(GraphicsMemoryAllocationType::Texture {
                                        width: (img.width / 16) as usize,
                                        height: (img.height / 16) as usize,
                                        depth: 256,
                                        is_3d_tex: true,
                                        flags: TexFlags::empty(),
                                    });
                                let mut image_3d_width = 0;
                                let mut image_3d_height = 0;
                                if !texture_2d_to_3d(
                                    &tp,
                                    img.data,
                                    img.width as usize,
                                    img.height as usize,
                                    4,
                                    16,
                                    16,
                                    mem.as_mut_slice(),
                                    &mut image_3d_width,
                                    &mut image_3d_height,
                                ) {
                                    panic!(
                                        "fatal error, could not convert 2d \
                                        texture to 2d array texture"
                                    );
                                }

                                // ALWAYS clear pixels of first tile, some mapres still have pixels in them
                                mem.as_mut_slice()[0..image_3d_width * image_3d_height * 4]
                                    .iter_mut()
                                    .for_each(|byte| *byte = 0);

                                (i, (image_3d_width, image_3d_height), mem, file.clone())
                            })
                            .collect()
                    },
                    || {
                        map.resources
                            .sounds
                            .into_par_iter()
                            .map(|i| {
                                let file = resources.get(&i.blake3_hash).unwrap();
                                let mut mem = sound_mt.mem_alloc(file.len());
                                mem.as_mut_slice().copy_from_slice(&file);
                                (i, mem, file.clone())
                            })
                            .collect()
                    }
                )
            });

        // sound scene
        let scene = scene_handle.create();
        let global_sound_listener = scene.sound_listener_handle.create(Default::default());

        // sound mem to sound objects
        let sound_objects: Vec<_> = sound_mems
            .into_iter()
            .map(|(i, mem, file)| (i, scene.sound_object_handle.create(mem), file))
            .collect();

        // load layers into vram
        enum MapLayerBuffer {
            Abritrary(Vec<u8>),
            Tile {
                buffer: ClientMapBufferTileLayer,
                layer: MapLayerTile,
            },
            Quad {
                buffer: ClientMapBufferQuadLayer,
                layer: MapLayerQuad,
            },
            Sound(MapLayerSound),
        }
        type GroupBuffers = Vec<(Vec<MapLayerBuffer>, MapGroupAttr, String)>;
        let upload_design_group_buffer = |groups: Vec<MapGroup>| -> GroupBuffers {
            groups
                .into_par_iter()
                .map(|group| {
                    (
                        group
                            .layers
                            .into_par_iter()
                            .map(|layer| match layer {
                                MapLayer::Abritrary(layer) => MapLayerBuffer::Abritrary(layer),
                                MapLayer::Tile(layer) => MapLayerBuffer::Tile {
                                    buffer: upload_design_tile_layer_buffer(
                                        &graphics_mt,
                                        &layer.tiles,
                                        layer.attr.width,
                                        layer.attr.height,
                                        layer.attr.image_array.is_some(),
                                    ),
                                    layer,
                                },
                                MapLayer::Quad(layer) => MapLayerBuffer::Quad {
                                    buffer: upload_design_quad_layer_buffer(
                                        &graphics_mt,
                                        &layer.attr,
                                        &layer.quads,
                                    ),
                                    layer,
                                },
                                MapLayer::Sound(layer) => MapLayerBuffer::Sound(layer),
                            })
                            .collect(),
                        group.attr,
                        group.name,
                    )
                })
                .collect()
        };
        let (physics_layers, background, foreground): (
            Vec<(ClientMapBufferPhysicsTileLayer, MapLayerPhysics)>,
            _,
            _,
        ) = tp.install(|| {
            join_all!(
                || map
                    .groups
                    .physics
                    .layers
                    .into_par_iter()
                    .map(|layer| {
                        (
                            upload_physics_layer_buffer(
                                &graphics_mt,
                                map.groups.physics.attr.width,
                                map.groups.physics.attr.height,
                                layer.as_ref().tiles_ref(),
                            ),
                            layer,
                        )
                    })
                    .collect(),
                || upload_design_group_buffer(map.groups.background),
                || upload_design_group_buffer(map.groups.foreground)
            )
        });

        let upload_design_group = |groups: GroupBuffers| {
            groups
                .into_iter()
                .map(|(layers, attr, name)| EditorGroup {
                    layers: layers
                        .into_iter()
                        .map(|layer| match layer {
                            MapLayerBuffer::Abritrary(layer) => {
                                EditorLayer::Abritrary(EditorLayerArbitrary {
                                    buf: layer,
                                    user: EditorArbitraryLayerProps {
                                        attr: Default::default(),
                                    },
                                })
                            }
                            MapLayerBuffer::Tile { layer, buffer } => {
                                EditorLayer::Tile(EditorLayerTile {
                                    user: EditorTileLayerProps {
                                        visuals: finish_design_tile_layer_buffer(
                                            buffer_object_handle,
                                            backend_handle,
                                            buffer,
                                        ),
                                        attr: EditorCommonGroupOrLayerAttr::default(),
                                        selected: Default::default(),
                                    },
                                    layer,
                                })
                            }
                            MapLayerBuffer::Quad { layer, buffer } => {
                                EditorLayer::Quad(EditorLayerQuad {
                                    user: EditorQuadLayerProps {
                                        visuals: finish_design_quad_layer_buffer(
                                            buffer_object_handle,
                                            backend_handle,
                                            buffer,
                                        ),
                                        attr: EditorCommonGroupOrLayerAttr::default(),
                                        selected: Default::default(),
                                    },
                                    layer,
                                })
                            }
                            MapLayerBuffer::Sound(layer) => EditorLayer::Sound(EditorLayerSound {
                                user: EditorSoundLayerProps {
                                    sounds: SoundLayerSounds::default(),
                                    attr: Default::default(),
                                    selected: Default::default(),
                                },
                                layer,
                            }),
                        })
                        .collect(),
                    attr: attr,
                    name: name,
                    user: EditorGroupProps::default(),
                })
                .collect()
        };
        let map =
            EditorMap {
                user: EditorMapProps {
                    options: Default::default(),
                    ui_values: Default::default(),
                    sound_scene: scene,
                    global_sound_listener,
                    time: Duration::ZERO,
                    time_scale: 0,
                },
                resources: EditorResources {
                    images: image_mems
                        .into_iter()
                        .map(|(i, img, mem, file)| EditorImage {
                            user: EditorResource {
                                user: texture_handle
                                    .load_texture(
                                        img.width as usize,
                                        img.height as usize,
                                        ImageFormat::Rgba,
                                        mem,
                                        TexFormat::RGBA,
                                        TexFlags::empty(),
                                        &i.name,
                                    )
                                    .unwrap(),
                                file: file.into(),
                            },
                            def: i,
                        })
                        .collect(),
                    image_arrays: image_array_mems
                        .into_iter()
                        .map(|(i, (w, h), mem, file)| EditorImage2dArray {
                            user: EditorResource {
                                user: texture_handle
                                    .load_texture_3d(
                                        w,
                                        h,
                                        256,
                                        ImageFormat::Rgba,
                                        mem,
                                        TexFormat::RGBA,
                                        TexFlags::empty(),
                                        &i.name,
                                    )
                                    .unwrap(),
                                file: file.into(),
                            },
                            def: i,
                        })
                        .collect(),
                    sounds: sound_objects
                        .into_iter()
                        .map(|(i, s, file)| EditorSound {
                            def: i,
                            user: EditorResource {
                                user: s,
                                file: file.into(),
                            },
                        })
                        .collect(),
                    user: (),
                },
                animations: EditorAnimations {
                    pos: map
                        .animations
                        .pos
                        .into_iter()
                        .map(|pos| EditorPosAnimation {
                            def: pos,
                            user: Default::default(),
                        })
                        .collect(),
                    color: map
                        .animations
                        .color
                        .into_iter()
                        .map(|color| EditorColorAnimation {
                            def: color,
                            user: Default::default(),
                        })
                        .collect(),
                    sound: map
                        .animations
                        .sound
                        .into_iter()
                        .map(|sound| EditorSoundAnimation {
                            def: sound,
                            user: Default::default(),
                        })
                        .collect(),
                    user: EditorAnimationsProps::default(),
                },
                groups: EditorGroups {
                    physics: EditorGroupPhysics {
                        layers: physics_layers
                            .into_iter()
                            .map(|(buffer, layer)| {
                                let user = EditorPhysicsLayerProps {
                                    visuals: finish_physics_layer_buffer(
                                        buffer_object_handle,
                                        backend_handle,
                                        buffer,
                                    ),
                                    attr: EditorCommonGroupOrLayerAttr::default(),
                                    selected: Default::default(),
                                };
                                match layer {
                                    MapLayerPhysics::Arbitrary(layer) => {
                                        EditorPhysicsLayer::Arbitrary(
                                            MapLayerArbitraryPhysicsSkeleton { buf: layer, user },
                                        )
                                    }
                                    MapLayerPhysics::Game(layer) => {
                                        EditorPhysicsLayer::Game(MapLayerTilePhysicsBaseSkeleton {
                                            def: layer,
                                            user,
                                        })
                                    }
                                    MapLayerPhysics::Front(layer) => {
                                        EditorPhysicsLayer::Front(MapLayerTilePhysicsBaseSkeleton {
                                            def: layer,
                                            user,
                                        })
                                    }
                                    MapLayerPhysics::Tele(layer) => {
                                        EditorPhysicsLayer::Tele(MapLayerTilePhysicsBaseSkeleton {
                                            def: layer,
                                            user,
                                        })
                                    }
                                    MapLayerPhysics::Speedup(layer) => EditorPhysicsLayer::Speedup(
                                        MapLayerTilePhysicsBaseSkeleton { def: layer, user },
                                    ),
                                    MapLayerPhysics::Switch(layer) => EditorPhysicsLayer::Switch(
                                        MapLayerTilePhysicsBaseSkeleton { def: layer, user },
                                    ),
                                    MapLayerPhysics::Tune(layer) => {
                                        EditorPhysicsLayer::Tune(MapLayerTilePhysicsBaseSkeleton {
                                            def: layer,
                                            user,
                                        })
                                    }
                                }
                            })
                            .collect(),
                        attr: map.groups.physics.attr,
                        user: EditorPhysicsGroupProps::default(),
                    },
                    background: upload_design_group(background),
                    foreground: upload_design_group(foreground),
                    user: EditorGroupsProps {
                        pos: Default::default(),
                        zoom: 1.0,
                    },
                },
                config: EditorConfig {
                    def: map.config,
                    user: (),
                },
                meta: EditorMetadata {
                    def: map.meta,
                    user: (),
                },
            };
        map
    }

    fn map_to_editor_map(&self, map: Map, resources: HashMap<Hash, Vec<u8>>) -> EditorMap {
        Self::map_to_editor_map_impl(
            self.graphics_mt.clone(),
            self.sound_mt.clone(),
            &self.thread_pool,
            &self.scene_handle,
            &self.backend_handle,
            &self.buffer_object_handle,
            &self.texture_handle,
            map,
            resources,
        )
    }

    pub fn load_legacy_map(&mut self, path: &Path) {
        let name = path.file_stem().unwrap().to_string_lossy().to_string();

        let tp = self.thread_pool.clone();
        let map = legacy_to_new(path, &self.io.clone().into(), &tp, true)
            .map_err(|err| anyhow!("Loading legacy map loading failed: {err}"))
            .unwrap();

        let resources: Vec<_> = map
            .resources
            .images
            .into_iter()
            .map(|res| (res.blake3_hash, res.buf))
            .chain(
                map.resources
                    .sounds
                    .into_iter()
                    .map(|res| (res.blake3_hash, res.buf)),
            )
            .collect();
        let map = self.map_to_editor_map(map.map, resources.into_iter().collect());

        let server = EditorServer::new(&self.sys, None, None, "".into());
        let client = EditorClient::new(
            &self.sys,
            &format!("127.0.0.1:{}", server.port),
            NetworkClientCertCheckMode::CheckByCert { cert: &server.cert },
            self.notifications.clone(),
            "".into(),
            true,
        );

        self.tabs.insert(
            name.clone(),
            EditorTab {
                map,
                map_render: RenderMap::new(
                    &self.backend_handle,
                    &self.canvas_handle,
                    &self.stream_handle,
                ),
                server: Some(server),
                client,
            },
        );
        self.active_tab = name;
    }

    pub fn load_map_impl(
        &mut self,
        path: &Path,
        cert: Option<NetworkServerCertMode>,
        port: Option<u16>,
        password: Option<String>,
    ) {
        let name = path.file_stem().unwrap().to_string_lossy().to_string();

        let fs = self.io.fs.clone();
        let tp = self.thread_pool.clone();
        let path = path.to_path_buf();
        let (map, resources) = self
            .io
            .io_batcher
            .spawn(async move {
                let file = fs.open_file(&path).await?;
                let map = Map::read(&file, &tp).unwrap();
                #[derive(Debug, PartialEq, Clone, Copy)]
                enum ReadFileTy {
                    Image,
                    Sound,
                }
                let mut resource_files: HashMap<Hash, Vec<u8>> = Default::default();
                for (ty, i) in map
                    .resources
                    .images
                    .iter()
                    .map(|i| (ReadFileTy::Image, i))
                    .chain(
                        map.resources
                            .image_arrays
                            .iter()
                            .map(|i| (ReadFileTy::Image, i)),
                    )
                    .chain(map.resources.sounds.iter().map(|i| (ReadFileTy::Sound, i)))
                {
                    if let std::collections::hash_map::Entry::Vacant(e) =
                        resource_files.entry(i.blake3_hash)
                    {
                        let file = fs
                            .open_file(
                                format!(
                                    "map/resources/{}/{}_{}.{}",
                                    if ty == ReadFileTy::Image {
                                        "images"
                                    } else {
                                        "sounds"
                                    },
                                    i.name,
                                    fmt_hash(&i.blake3_hash),
                                    i.ty
                                )
                                .as_ref(),
                            )
                            .await?;

                        e.insert(file);
                    }
                }

                Ok((map, resource_files))
            })
            .get_storage()
            .unwrap();

        let map = self.map_to_editor_map(map, resources);

        let server = EditorServer::new(&self.sys, cert, port, password.clone().unwrap_or_default());
        let client = EditorClient::new(
            &self.sys,
            &format!("127.0.0.1:{}", server.port),
            NetworkClientCertCheckMode::CheckByCert { cert: &server.cert },
            self.notifications.clone(),
            password.unwrap_or_default(),
            true,
        );

        self.tabs.insert(
            name.clone(),
            EditorTab {
                map,
                map_render: RenderMap::new(
                    &self.backend_handle,
                    &self.canvas_handle,
                    &self.stream_handle,
                ),
                server: Some(server),
                client,
            },
        );
        self.active_tab = name;
    }

    pub fn load_map(&mut self, path: &Path) {
        self.load_map_impl(path, None, None, None)
    }

    pub fn save_map_legacy(&mut self, path: &Path) {
        if let Some(_) = self.tabs.get(&self.active_tab) {
            let mut twmap_path = path.to_path_buf();
            twmap_path.set_extension(".twmap");
            let task = self.save_map(&twmap_path);
            task.unwrap().get_storage().unwrap();

            let map_legacy =
                new_to_legacy(&twmap_path, &self.io.clone().into(), &self.thread_pool).unwrap();
            let fs = self.io.fs.clone();
            let path = path.to_path_buf();
            self.io.io_batcher.spawn_without_lifetime(async move {
                fs.write_file(&path, map_legacy.map).await?;
                Ok(())
            })
        }
    }

    pub fn save_map(&mut self, path: &Path) -> Option<IOBatcherTask<()>> {
        if let Some(tab) = self.tabs.get(&self.active_tab) {
            let map: Map = tab.map.clone().into();
            let tp = self.thread_pool.clone();
            let fs = self.io.fs.clone();
            Some(self.io.io_batcher.spawn(async move {
                let mut file: Vec<u8> = Default::default();
                map.write(&mut file, &tp)?;
                fs.write_file("test.twmap".as_ref(), file).await?;
                Ok(())
            }))
        } else {
            None
        }
    }

    fn update(&mut self) {
        let time_now = self.sys.time_get_nanoseconds();
        let time_diff = time_now - self.last_time;
        self.last_time = time_now;
        for (_, tab) in &mut self.tabs {
            tab.map.user.time += time_diff * tab.map.user.time_scale;

            if let Some(EditorEventOverwriteMap { map, resources }) = tab.client.update(
                &self.thread_pool,
                &self.sound_mt,
                &self.graphics_mt,
                &self.buffer_object_handle,
                &self.backend_handle,
                &self.texture_handle,
                &mut tab.map,
            ) {
                let map = Map::read(&map, &self.thread_pool).unwrap();
                tab.map = Self::map_to_editor_map_impl(
                    self.graphics_mt.clone(),
                    self.sound_mt.clone(),
                    &self.thread_pool,
                    &self.scene_handle,
                    &self.backend_handle,
                    &self.buffer_object_handle,
                    &self.texture_handle,
                    map,
                    resources,
                );
            }
            if let Some(server) = &mut tab.server {
                server.update(
                    &self.thread_pool,
                    &self.sound_mt,
                    &self.graphics_mt,
                    &self.buffer_object_handle,
                    &self.backend_handle,
                    &self.texture_handle,
                    &mut tab.map,
                );
            }
        }
    }

    fn render_design_layer<AS: HiarcTrait, A: HiarcTrait>(
        &self,
        map_render: &RenderMap,
        map: &EditorMap,
        animations: &AnimationsSkeleton<AS, A>,
        group: &EditorGroup,
        layer: &EditorLayer,
        as_tile_numbers: Option<&TextureContainer2dArray>,
    ) {
        map_render.render_layer(
            animations,
            &map.resources,
            &ConfigMap::default(),
            &map.game_camera(),
            &map.game_state_info(),
            &map.user.time,
            &group.attr,
            layer,
            match layer {
                MapLayerSkeleton::Abritrary(_) | MapLayerSkeleton::Sound(_) => None,
                MapLayerSkeleton::Tile(layer) => {
                    if let Some(numbers) = as_tile_numbers {
                        Some(ForcedTexture::TileLayer(numbers))
                    } else {
                        if let Some(EditorTileLayerPropsSelection {
                            image_2d_array_selection_open:
                                Some(ResourceSelection {
                                    hovered_resource: Some(index),
                                }),
                            ..
                        }) = layer.user.selected
                        {
                            index
                                .map(|index| {
                                    map.resources
                                        .image_arrays
                                        .get(index)
                                        .map(|res| ForcedTexture::TileLayer(&res.user.user))
                                })
                                .unwrap_or_else(|| {
                                    Some(ForcedTexture::TileLayer(&self.fake_texture_array))
                                })
                        } else {
                            if layer.layer.attr.image_array.is_none() {
                                Some(ForcedTexture::TileLayer(&self.fake_texture_array))
                            } else {
                                None
                            }
                        }
                    }
                }
                MapLayerSkeleton::Quad(layer) => {
                    if let Some(EditorQuadLayerPropsPropsSelection {
                        image_selection_open:
                            Some(ResourceSelection {
                                hovered_resource: Some(index),
                            }),
                        ..
                    }) = layer.user.selected
                    {
                        index
                            .map(|index| {
                                map.resources
                                    .images
                                    .get(index)
                                    .map(|res| ForcedTexture::QuadLayer(&res.user.user))
                            })
                            .unwrap_or_else(|| Some(ForcedTexture::QuadLayer(&self.fake_texture)))
                    } else {
                        if layer.layer.attr.image.is_none() {
                            Some(ForcedTexture::QuadLayer(&self.fake_texture))
                        } else {
                            None
                        }
                    }
                }
            },
        );
    }

    fn render_design_groups(
        &self,
        map_render: &RenderMap,
        map: &EditorMap,
        groups: &Vec<EditorGroup>,
        tile_numbers_texture: TextureContainer2dArray,
    ) {
        for group in groups.iter().filter(|group| !group.editor_attr().hidden) {
            for layer in group.layers.iter() {
                if !layer.editor_attr().hidden {
                    if map.user.ui_values.animations_panel_open {
                        self.render_design_layer(
                            map_render,
                            map,
                            &map.animations.user.animations,
                            group,
                            layer,
                            None,
                        );
                    } else {
                        self.render_design_layer(
                            map_render,
                            map,
                            &map.animations,
                            group,
                            layer,
                            None,
                        );
                    }
                    if layer.editor_attr().active && map.user.options.show_tile_numbers {
                        self.render_design_layer(
                            map_render,
                            map,
                            &map.animations,
                            group,
                            layer,
                            Some(&tile_numbers_texture),
                        );
                    }
                    if let MapLayerSkeleton::Sound(layer) = layer {
                        let time = if map.user.ui_values.animations_panel_open {
                            map.user.ui_values.timeline.time()
                        } else {
                            map.user.time
                        };
                        map_render.sound.handle_sound_layer(
                            &map.resources.sounds,
                            layer,
                            &Camera {
                                pos: map.groups.user.pos,
                                zoom: map.groups.user.zoom,
                                animation_ticks_passed: (time.as_millis() / (1000 / 50)).max(1)
                                    as u64,
                            },
                        );
                    }
                } else {
                    if let MapLayerSkeleton::Sound(layer) = layer {
                        layer.user.sounds.stop_all();
                    }
                }
            }
        }
    }

    fn render_physics_layer(
        entities_container: &mut EntitiesContainer,
        map_render: &RenderMap,
        map: &EditorMap,
        layer: &EditorPhysicsLayer,
        as_tile_numbers: Option<&TextureContainer2dArray>,
    ) {
        let time = if map.user.ui_values.animations_panel_open {
            map.user.ui_values.timeline.time()
        } else {
            map.user.time
        };
        map_render.render_physics_layer(
            &map.animations,
            &map.game_state_info(),
            entities_container,
            layer,
            &Camera {
                pos: map.groups.user.pos,
                zoom: map.groups.user.zoom,
                animation_ticks_passed: (time.as_millis() / (1000 / 50)).max(1) as u64,
            },
            &map.user.time,
            100,
            as_tile_numbers.map(|tex| ForcedTexture::TileLayer(tex)),
        );
    }

    fn render_physics_group(
        entities_container: &mut EntitiesContainer,
        map_render: &RenderMap,
        map: &EditorMap,
        group: &EditorGroupPhysics,
        tile_numbers_texture: TextureContainer2dArray,
    ) {
        if group.editor_attr().hidden {
            return;
        }
        for layer in group
            .layers
            .iter()
            .filter(|&layer| !layer.user().attr.hidden)
        {
            Self::render_physics_layer(entities_container, map_render, map, layer, None);

            if layer.editor_attr().active && map.user.options.show_tile_numbers {
                Self::render_physics_layer(
                    entities_container,
                    map_render,
                    map,
                    layer,
                    Some(&tile_numbers_texture),
                );
            }
        }
    }

    /// brushes, moving camera etc.
    fn handle_world(&mut self, unused_rect: egui::Rect) {
        // handle middle mouse click
        if self.latest_pointer.middle_down() {
            let active_tab = self.tabs.get_mut(&self.active_tab);
            if let Some(tab) = active_tab {
                if let Some(middle_down_pointer) = &self.middle_down_pointer_pos {
                    let pos = self.current_pointer_pos;
                    let old_pos = middle_down_pointer;

                    let zoom = tab.map.groups.user.zoom;
                    let pos = ui_pos_to_world_pos(
                        &self.canvas_handle,
                        zoom,
                        vec2::new(pos.x, pos.y),
                        0.0,
                        0.0,
                        0.0,
                        0.0,
                        100.0,
                        100.0,
                    );
                    let old_pos = ui_pos_to_world_pos(
                        &self.canvas_handle,
                        zoom,
                        vec2::new(old_pos.x, old_pos.y),
                        0.0,
                        0.0,
                        0.0,
                        0.0,
                        100.0,
                        100.0,
                    );

                    tab.map.groups.user.pos.x -= pos.x - old_pos.x;
                    tab.map.groups.user.pos.y -= pos.y - old_pos.y;
                }
                self.middle_down_pointer_pos = Some(self.current_pointer_pos);
            }
        } else {
            self.middle_down_pointer_pos = None;
        }

        let active_tab = self.tabs.get_mut(&self.active_tab);
        if let Some(tab) = active_tab {
            // handle zoom
            if self.current_scroll_delta.y.abs() > 0.01 {
                let zoom_ranges = [
                    (0.0..0.6, 0.1),
                    (0.6..1.0, 0.2),
                    (1.0..5.0, 0.5),
                    (5.0..10.0, 1.0),
                    (10.0..f32::MAX, 10.0),
                ];
                // zoom in => non-inclusive range, zoom out => inclusive range
                let (_, step) = zoom_ranges
                    .iter()
                    .find(|&(zoom_range, _)| {
                        if self.current_scroll_delta.y.is_sign_negative() {
                            (zoom_range.start..zoom_range.end)
                                .contains(&tab.map.groups.user.zoom.abs())
                        } else {
                            (zoom_range.start..=zoom_range.end)
                                .contains(&tab.map.groups.user.zoom.abs())
                        }
                    })
                    .unwrap();
                tab.map.groups.user.zoom = (tab.map.groups.user.zoom
                    + step * -self.current_scroll_delta.y.signum())
                .clamp(0.2, 200.0);
            }

            // change active tool set
            match tab.map.active_layer() {
                Some(layer) => match layer {
                    EditorLayerUnionRef::Physics { .. } => {
                        if !matches!(self.tools.active_tool, ActiveTool::Tiles(_)) {
                            self.tools.active_tool = ActiveTool::Tiles(ActiveToolTiles::Brush);
                        }
                    }
                    EditorLayerUnionRef::Design { layer, .. } => match layer {
                        MapLayerSkeleton::Abritrary(_) => {}
                        MapLayerSkeleton::Tile(_) => {
                            if !matches!(self.tools.active_tool, ActiveTool::Tiles(_)) {
                                self.tools.active_tool = ActiveTool::Tiles(ActiveToolTiles::Brush);
                            }
                        }
                        MapLayerSkeleton::Quad(_) => {
                            if !matches!(self.tools.active_tool, ActiveTool::Quads(_)) {
                                self.tools.active_tool = ActiveTool::Quads(ActiveToolQuads::Brush);
                            }
                        }
                        MapLayerSkeleton::Sound(_) => {
                            todo!()
                        }
                    },
                },
                None => { // simply do nothing
                }
            }

            match &self.tools.active_tool {
                ActiveTool::Tiles(tool) => self.tools.tiles.update(
                    tool,
                    &self.thread_pool,
                    &self.graphics_mt,
                    &self.buffer_object_handle,
                    &self.backend_handle,
                    &self.stream_handle,
                    &self.canvas_handle,
                    &mut self.entities_container,
                    &self.fake_texture_array,
                    &tab.map,
                    &self.latest_pointer,
                    &self.latest_keys_down,
                    &self.latest_modifiers,
                    &self.current_pointer_pos,
                    &unused_rect,
                    &mut tab.client,
                ),
                ActiveTool::Quads(tool) => self.tools.quads.update(
                    tool,
                    &self.stream_handle,
                    &self.graphics_mt,
                    &self.buffer_object_handle,
                    &self.backend_handle,
                    &self.canvas_handle,
                    &tab.map,
                    &self.fake_texture,
                    &self.latest_pointer,
                    &self.current_pointer_pos,
                    &self.latest_modifiers,
                    &mut tab.client,
                ),
                ActiveTool::Sounds(tool) => self.tools.sounds.update(
                    tool,
                    &self.stream_handle,
                    &self.graphics_mt,
                    &self.buffer_object_handle,
                    &self.backend_handle,
                    &self.canvas_handle,
                    &tab.map,
                    &self.fake_texture,
                    &self.latest_pointer,
                    &self.current_pointer_pos,
                    &self.latest_modifiers,
                    &mut tab.client,
                ),
            }
        }
    }

    fn render_tools(&mut self) {
        let active_tab = self.tabs.get_mut(&self.active_tab);
        if let Some(tab) = active_tab {
            // change active tool set
            match tab.map.active_layer() {
                Some(layer) => match layer {
                    EditorLayerUnionRef::Physics { .. } => {
                        if !matches!(self.tools.active_tool, ActiveTool::Tiles(_)) {
                            self.tools.active_tool = ActiveTool::Tiles(ActiveToolTiles::Brush);
                        }
                    }
                    EditorLayerUnionRef::Design { layer, .. } => match layer {
                        MapLayerSkeleton::Abritrary(_) => {}
                        MapLayerSkeleton::Tile(_) => {
                            if !matches!(self.tools.active_tool, ActiveTool::Tiles(_)) {
                                self.tools.active_tool = ActiveTool::Tiles(ActiveToolTiles::Brush);
                            }
                        }
                        MapLayerSkeleton::Quad(_) => {
                            if !matches!(self.tools.active_tool, ActiveTool::Quads(_)) {
                                self.tools.active_tool = ActiveTool::Quads(ActiveToolQuads::Brush);
                            }
                        }
                        MapLayerSkeleton::Sound(_) => {
                            if !matches!(self.tools.active_tool, ActiveTool::Sounds(_)) {
                                self.tools.active_tool =
                                    ActiveTool::Sounds(ActiveToolSounds::Brush);
                            }
                        }
                    },
                },
                None => {
                    // simply do nothing
                }
            }

            match &self.tools.active_tool {
                ActiveTool::Tiles(tool) => self.tools.tiles.render(
                    tool,
                    &self.thread_pool,
                    &self.graphics_mt,
                    &self.buffer_object_handle,
                    &self.backend_handle,
                    &self.stream_handle,
                    &self.canvas_handle,
                    &mut self.entities_container,
                    &self.fake_texture_array,
                    &tab.map,
                    &self.latest_pointer,
                    &self.latest_keys_down,
                    &self.current_pointer_pos,
                    &self.latest_unused_rect,
                    &mut tab.client,
                ),
                ActiveTool::Quads(tool) => self.tools.quads.render(
                    tool,
                    &self.stream_handle,
                    &self.graphics_mt,
                    &self.buffer_object_handle,
                    &self.backend_handle,
                    &self.canvas_handle,
                    &tab.map,
                    &self.latest_pointer,
                    &self.current_pointer_pos,
                    &mut tab.client,
                ),
                ActiveTool::Sounds(tool) => self.tools.sounds.render(
                    tool,
                    &self.stream_handle,
                    &self.graphics_mt,
                    &self.buffer_object_handle,
                    &self.backend_handle,
                    &self.canvas_handle,
                    &tab.map,
                    &self.latest_pointer,
                    &self.current_pointer_pos,
                    &mut tab.client,
                ),
            }
        }
    }

    fn clone_anim_from_map<A, AP: DeserializeOwned + PartialOrd + Clone>(
        animations: &mut Vec<AnimBaseSkeleton<(), AP>>,
        from: &Vec<AnimBaseSkeleton<A, AP>>,
    ) where
        AnimBaseSkeleton<A, AP>: Into<AnimBase<AP>>,
    {
        animations.clear();
        animations.extend(from.iter().map(|anim| AnimBaseSkeleton {
            def: anim.def.clone().into(),
            user: (),
        }));
    }

    fn add_fake_anim_point(tools: &mut Tools, map: &mut EditorMap) {
        Self::clone_anim_from_map(
            &mut map.animations.user.animations.color,
            &map.animations.color,
        );
        Self::clone_anim_from_map(&mut map.animations.user.animations.pos, &map.animations.pos);
        Self::clone_anim_from_map(
            &mut map.animations.user.animations.sound,
            &map.animations.sound,
        );

        let active_layer = map.groups.active_layer();
        if let (
            Some(EditorLayerUnionRef::Design {
                layer: EditorLayer::Quad(layer),
                ..
            }),
            ActiveTool::Quads(ActiveToolQuads::Selection),
            QuadSelection {
                range: Some(range),
                anim_point_pos,
                ..
            },
        ) = (
            &active_layer,
            &tools.active_tool,
            &mut tools.quads.selection,
        ) {
            let range = range.indices_checked(layer);

            if !range.is_empty() {
                let (_, quad) = range
                    .iter()
                    .next()
                    .map(|(i, q)| (*i, (*q).clone()))
                    .unwrap();

                if let Some(pos_anim) = quad.pos_anim {
                    enum ReplOrInsert {
                        Repl(usize),
                        Insert(usize),
                    }
                    let t = map.user.ui_values.timeline.time();
                    if let Some(index) = map.animations.user.animations.pos[pos_anim]
                        .def
                        .points
                        .iter()
                        .enumerate()
                        .find_map(|(p, point)| {
                            if point.time > t {
                                Some(ReplOrInsert::Insert(p))
                            } else if point.time == t {
                                Some(ReplOrInsert::Repl(p))
                            } else {
                                None
                            }
                        })
                    {
                        match index {
                            ReplOrInsert::Repl(index) => {
                                map.animations.user.animations.pos[pos_anim].def.points[index] =
                                    AnimPointPos {
                                        time: t,
                                        curve_type: AnimPointCurveType::Linear,
                                        value: anim_point_pos.value,
                                    };
                            }
                            ReplOrInsert::Insert(index) => {
                                map.animations.user.animations.pos[pos_anim]
                                    .def
                                    .points
                                    .insert(
                                        index,
                                        AnimPointPos {
                                            time: t,
                                            curve_type: AnimPointCurveType::Linear,
                                            value: anim_point_pos.value,
                                        },
                                    );
                            }
                        }
                    }
                }
            }
        }
    }

    pub fn render_world(&mut self) {
        if let Some(tab) = self.tabs.get_mut(&self.active_tab) {
            // update anim if anim panel is open and e.g. quad selection is active
            if tab.map.user.ui_values.animations_panel_open {
                Self::add_fake_anim_point(&mut self.tools, &mut tab.map);
            }
        }
        let active_tab = self.tabs.get(&self.active_tab);
        if let Some(tab) = active_tab {
            let tile_numbers_texture = self
                .entities_container
                .get_or_default(&"default".into())
                .text_overlay_bottom
                .clone();
            // we use sound
            tab.map.user.sound_scene.stay_active();
            // bg
            self.render_design_groups(
                &tab.map_render,
                &tab.map,
                &tab.map.groups.background,
                tile_numbers_texture.clone(),
            );
            // physics
            Self::render_physics_group(
                &mut self.entities_container,
                &tab.map_render,
                &tab.map,
                &tab.map.groups.physics,
                tile_numbers_texture.clone(),
            );
            // fg
            self.render_design_groups(
                &tab.map_render,
                &tab.map,
                &tab.map.groups.foreground,
                tile_numbers_texture,
            );
        }
    }

    fn render_ui(
        &mut self,
        input: egui::RawInput,
        config: &ConfigEngine,
    ) -> (Option<egui::Rect>, egui::PlatformOutput) {
        let active_tab = self.tabs.get_mut(&self.active_tab);
        let mut unused_rect: Option<egui::Rect> = None;
        let egui_output = self.ui.render(EditorUiRenderPipe {
            cur_time: self.sys.time_get_nanoseconds(),
            events: &mut self.events,
            config,
            inp: input,
            editor_tab: active_tab,
            ui_events: &mut self.ui_events,
            unused_rect: &mut unused_rect,
            tools: &mut self.tools,
            auto_mapper: &mut self.auto_mapper,
            io: &self.io,
        });

        // handle ui events
        for ev in std::mem::take(&mut self.ui_events) {
            match ev {
                EditorUiEvent::OpenFile { name } => self.load_map(&name),
                EditorUiEvent::SaveFile { name } => {
                    let _ = self.save_map(&name);
                }
                EditorUiEvent::HostMap {
                    map_path,
                    port,
                    password,
                    private_key_der,
                    pub_key_der,
                } => {
                    self.load_map_impl(
                        map_path.as_ref(),
                        Some(NetworkServerCertMode::FromPrivatePubDer {
                            pub_key: &pub_key_der,
                            private_key: &private_key_der,
                        }),
                        Some(port),
                        Some(password),
                    );
                }
                EditorUiEvent::Join {
                    ip_port,
                    cert_hash,
                    password,
                } => self.new_map(
                    "loading",
                    Some(
                        (0..cert_hash.len())
                            .step_by(2)
                            .map(|i| u8::from_str_radix(&cert_hash[i..i + 2], 16).unwrap())
                            .collect::<Vec<_>>()
                            .try_into()
                            .unwrap(),
                    ),
                    Some(ip_port),
                    Some(password),
                ),
            }
        }
        (unused_rect, egui_output)
    }
}

pub trait EditorInterface {
    fn render(
        &mut self,
        input: egui::RawInput,
        config: &ConfigEngine,
    ) -> Option<egui::PlatformOutput>;
}

impl EditorInterface for Editor {
    fn render(
        &mut self,
        input: egui::RawInput,
        config: &ConfigEngine,
    ) -> Option<egui::PlatformOutput> {
        // do an update
        self.update();

        // then render the map
        self.render_world();

        // if msaa is enabled, consume them now
        self.backend_handle.consumble_multi_samples();

        // render the tools directly after the world
        // the handling/update of the tools & world happens after the UI tho
        self.render_tools();

        // then render the UI above it
        let (unused_rect, ui_output) = self.render_ui(input, config);

        // outside of the UI / inside of the world, handle brushes etc.
        // working with egui directly doesn't feel great... copy some interesting input values
        let (latest_pointer, scroll_delta, keys, modifiers) =
            self.ui.ui.context.egui_ctx.input(|inp| {
                (
                    inp.pointer.clone(),
                    inp.raw_scroll_delta.clone(),
                    inp.keys_down.clone(),
                    inp.modifiers.clone(),
                )
            });
        if unused_rect.is_some_and(|unused_rect| {
            unused_rect.contains(
                latest_pointer
                    .interact_pos()
                    .unwrap_or(self.current_pointer_pos),
            )
        }) {
            self.latest_keys_down = keys;
            self.latest_modifiers = modifiers;
            self.latest_pointer = latest_pointer;
            self.latest_unused_rect = unused_rect.unwrap();
            self.current_scroll_delta = scroll_delta;
            self.current_pointer_pos = self
                .latest_pointer
                .latest_pos()
                .unwrap_or(self.current_pointer_pos);
            self.handle_world(self.latest_unused_rect);
        } else {
            self.current_scroll_delta = Default::default();
        }

        if !ui_output.copied_text.is_empty() {
            dbg!(&ui_output.copied_text);
        }
        Some(ui_output)
    }
}
