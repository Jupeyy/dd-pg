pub mod graphic_border_tile;
pub mod graphic_tile;

use std::{borrow::BorrowMut, collections::HashMap, ops::Range, sync::Arc};

use graphics::{
    graphics_mt::GraphicsMultiThreaded,
    handles::{
        backend::backend::GraphicsBackendHandle,
        buffer_object::buffer_object::{BufferObject, GraphicsBufferObjectHandle},
        texture::texture::{TextureContainer, TextureContainer2dArray},
    },
};
use hiarc::{hiarc_safer_rc_refcell, Hiarc};
use map::{
    map::{
        groups::{
            layers::{
                design::{MapLayer, MapLayerQuadsAttrs, Quad},
                physics::MapLayerPhysics,
                tiles::{MapTileLayerPhysicsTilesRef, TileBase, TileFlags},
            },
            MapGroup,
        },
        Map,
    },
    skeleton::groups::layers::{
        design::{MapLayerQuadSkeleton, MapLayerTileSkeleton},
        physics::{
            MapLayerArbitraryPhysicsSkeleton, MapLayerPhysicsSkeleton,
            MapLayerTilePhysicsBaseSkeleton,
        },
    },
    types::NonZeroU16MinusOne,
};
use rayon::{
    iter::IntoParallelRefIterator,
    prelude::{IndexedParallelIterator, ParallelIterator},
    slice::ParallelSliceMut,
};
use shared_base::mapdef_06::{TileNum, TILE_SWITCHTIMEDOPEN};

use math::math::vector::ivec2;

use graphics_types::{
    commands::CommandUpdateBufferObjectRegion,
    types::{GraphicsBackendMemory, GraphicsMemoryAllocationType},
};
use sound::{
    scene_object::SceneObject, sound_listener::SoundListener, sound_object::SoundObject,
    sound_play_handle::SoundPlayHandle,
};

use crate::map::map_with_visual::{
    MapVisualConfig, MapVisualImage2dArray, MapVisualLayerArbitrary, MapVisualLayerQuad,
    MapVisualLayerSound, MapVisualLayerTile, MapVisualMetadata, MapVisualPhysicsLayer,
    MapVisualProps,
};

use self::{
    graphic_border_tile::{
        add_border_tile, GraphicBorderTile, GraphicsBorderTilePos, GraphicsBorderTileTex,
    },
    graphic_tile::{add_tile, GraphicTile, GraphicsTilePos, GraphicsTileTex},
};

use super::map_with_visual::{
    MapVisual, MapVisualAnimations, MapVisualColorAnimation, MapVisualGroup, MapVisualGroups,
    MapVisualImage, MapVisualLayer, MapVisualPhysicsGroup, MapVisualPosAnimation,
    MapVisualResources, MapVisualSound, MapVisualSoundAnimation,
};

#[derive(Debug, Hiarc, Copy, Clone, Default)]
pub struct TileVisual {
    quad_count_and_is_drawable_flag: u32,
}

impl TileVisual {
    pub fn drawable(&self) -> bool {
        (self.quad_count_and_is_drawable_flag & 0x10000000) != 0
    }

    fn set_drawable(&mut self, drawable: bool) {
        self.quad_count_and_is_drawable_flag = (if drawable { 0x10000000 } else { 0 })
            | (self.quad_count_and_is_drawable_flag & 0xEFFFFFFF);
    }

    pub fn index_buffer_offset_quad(&self) -> usize {
        (self.quad_count_and_is_drawable_flag & 0xEFFFFFFF) as usize
            * 6
            * std::mem::size_of::<u32>()
    }

    fn set_index_buffer_offset_quad(&mut self, quad_count: u32) {
        self.quad_count_and_is_drawable_flag =
            quad_count | (self.quad_count_and_is_drawable_flag & 0x10000000);
    }

    fn add_index_buffer_offset_quad(&mut self, additional_quad_count: u32) {
        self.quad_count_and_is_drawable_flag =
            ((self.quad_count_and_is_drawable_flag & 0xEFFFFFFF) + additional_quad_count)
                | (self.quad_count_and_is_drawable_flag & 0x10000000);
    }
}

#[derive(Debug, Default, Clone, Hiarc)]
pub struct TileLayerVisualsBase {
    pub tiles_of_layer: Vec<TileVisual>,

    /// the size of all tiles that aren't part of the border
    pub buffer_size_all_tiles: usize,

    pub corner_top_left: TileVisual,
    pub corner_top_right: TileVisual,
    pub corner_bottom_right: TileVisual,
    pub corner_bottom_left: TileVisual,

    pub border_kill_tile: TileVisual, //end of map kill tile -- game layer only

    pub border_top: Vec<TileVisual>,
    pub border_left: Vec<TileVisual>,
    pub border_right: Vec<TileVisual>,
    pub border_bottom: Vec<TileVisual>,

    pub width: u32,
    pub height: u32,
    pub is_textured: bool,
}

impl TileLayerVisualsBase {
    pub fn new() -> Self {
        Default::default()
    }

    fn init(&mut self, width: u32, height: u32) -> bool {
        self.width = width;
        self.height = height;
        if width == 0 || height == 0 {
            return false;
        }
        if width as usize * height as usize >= u32::MAX as usize {
            return false;
        }

        self.tiles_of_layer
            .resize(height as usize * width as usize, TileVisual::default());

        self.border_top
            .resize(width as usize, TileVisual::default());
        self.border_bottom
            .resize(width as usize, TileVisual::default());
        self.border_left
            .resize(height as usize, TileVisual::default());
        self.border_right
            .resize(height as usize, TileVisual::default());

        true
    }
}

#[derive(Debug, Hiarc, Clone)]
pub struct TileLayerVisuals {
    pub base: TileLayerVisualsBase,
    pub buffer_object_index: Option<BufferObject>,
}

#[derive(Debug, Hiarc, Clone)]
pub struct PhysicsTileLayerOverlayVisuals {
    pub buffer_object: Option<BufferObject>,
    pub ty: MapRenderTextOverlayType,
}

#[derive(Debug, Hiarc, Clone)]
pub struct PhysicsTileLayerVisuals {
    pub base: TileLayerVisuals,
    pub overlay_buffer_objects: Vec<PhysicsTileLayerOverlayVisuals>,
}

#[derive(Copy, Clone, Default)]
pub struct QuadVisual {
    pub index_buffer_byte_offset: usize,
}

#[derive(Debug, Hiarc, Clone)]
pub struct QuadLayerVisuals {
    pub buffer_object_index: Option<BufferObject>,
}

#[hiarc_safer_rc_refcell]
#[derive(Debug, Hiarc, Default)]
pub struct SoundLayerSounds {
    /// `usize` here equals the sound index ([`map::map::MapLayerSound`])
    sound_plays: HashMap<usize, SoundPlayHandle>,
}

#[hiarc_safer_rc_refcell]
impl SoundLayerSounds {
    pub fn is_playing(&self, index: usize) -> bool {
        self.sound_plays.contains_key(&index)
    }
    pub fn play(&mut self, index: usize, play: SoundPlayHandle) {
        self.sound_plays.insert(index, play);
    }
    pub fn stop(&mut self, index: usize) {
        self.sound_plays.remove(&index);
    }
    pub fn stop_all(&mut self) {
        self.sound_plays.clear();
    }
}

#[repr(C)]
#[derive(Clone, Default)]
struct TmpQuadVertexTextured {
    x: f32,
    y: f32,
    center_x: f32,
    center_y: f32,
    r: u8,
    g: u8,
    b: u8,
    a: u8,
    u: f32,
    v: f32,
}

impl TmpQuadVertexTextured {
    fn copy_into_slice(&self, dest: &mut [u8], textured: bool) -> usize {
        let mut off: usize = 0;
        self.x.to_ne_bytes().iter().for_each(|byte| {
            dest[off] = *byte;
            off += 1;
        });
        self.y.to_ne_bytes().iter().for_each(|byte| {
            dest[off] = *byte;
            off += 1;
        });
        self.center_x.to_ne_bytes().iter().for_each(|byte| {
            dest[off] = *byte;
            off += 1;
        });
        self.center_y.to_ne_bytes().iter().for_each(|byte| {
            dest[off] = *byte;
            off += 1;
        });
        self.r.to_ne_bytes().iter().for_each(|byte| {
            dest[off] = *byte;
            off += 1;
        });
        self.g.to_ne_bytes().iter().for_each(|byte| {
            dest[off] = *byte;
            off += 1;
        });
        self.b.to_ne_bytes().iter().for_each(|byte| {
            dest[off] = *byte;
            off += 1;
        });
        self.a.to_ne_bytes().iter().for_each(|byte| {
            dest[off] = *byte;
            off += 1;
        });
        if textured {
            self.u.to_ne_bytes().iter().for_each(|byte| {
                dest[off] = *byte;
                off += 1;
            });
            self.v.to_ne_bytes().iter().for_each(|byte| {
                dest[off] = *byte;
                off += 1;
            });
        }
        off
    }
}

#[repr(C)]
#[derive(Clone, Default)]
struct TmpQuadTextured {
    vertices: [TmpQuadVertexTextured; 4],
}

impl TmpQuadTextured {
    fn copy_into_slice(&self, dest: &mut [u8], textured: bool) -> usize {
        let mut off: usize = 0;
        self.vertices.iter().for_each(|v| {
            off += v.copy_into_slice(dest.split_at_mut(off).1, textured);
        });
        off
    }
}

#[derive(Debug, Hiarc, Copy, Clone)]
pub enum MapRenderTextOverlayType {
    Top,
    Bottom,
    Center,
}

#[derive(Debug, Hiarc, Default, Copy, Clone)]
pub struct MapRenderInfo {
    pub group_index: usize,
    pub layer_index: usize,
}

#[derive(Debug, Default, Clone)]
pub struct MapPhysicsRenderInfo {
    pub layer_index: usize,
}

#[derive(Debug, Copy, Clone)]
pub enum MapRenderLayer {
    Tile(MapRenderInfo),
    Quad(MapRenderInfo),
}

impl MapRenderLayer {
    pub fn get_render_info(&self) -> &MapRenderInfo {
        match self {
            MapRenderLayer::Tile(render_info) => render_info,
            MapRenderLayer::Quad(render_info) => render_info,
        }
    }
}

#[derive(Default)]
pub struct ClientMapBufferedRenderProcess {
    pub background_render_layers: Vec<MapRenderLayer>,
    pub foreground_render_layers: Vec<MapRenderLayer>,
    pub physics_render_layers: Vec<MapPhysicsRenderInfo>,
}

#[derive(Debug, Hiarc, Default, Copy, Clone)]
pub struct MapSoundProcessInfo {
    pub group_index: usize,
    pub layer_index: usize,
}

#[derive(Default)]
pub struct ClientMapBufferedSoundProcess {
    pub background_sound_layers: Vec<MapSoundProcessInfo>,
    pub foreground_sound_layers: Vec<MapSoundProcessInfo>,
}

pub struct ClientMapBuffered {
    pub map_visual: MapVisual,
    pub render: ClientMapBufferedRenderProcess,
    pub sound: ClientMapBufferedSoundProcess,
}

#[derive(Debug, Default, Hiarc)]
pub struct ClientMapBufferTileLayer {
    mem: Option<GraphicsBackendMemory>,
    indices_count: u64,
    visuals: TileLayerVisualsBase,
    render_info: MapRenderInfo,
}

#[derive(Debug, Default)]
pub struct ClientMapBufferPhysicsTileLayer {
    mem: Option<GraphicsBackendMemory>,
    indices_count: u64,
    visuals: TileLayerVisualsBase,
    render_info: MapPhysicsRenderInfo,
    overlays: Vec<(MapRenderTextOverlayType, Option<GraphicsBackendMemory>)>,
}

pub type ClientMapBufferQuadLayer = (Option<GraphicsBackendMemory>, bool, u64, MapRenderInfo);

pub struct ClientMapBufferUploadData {
    pub bg_tile_layer_uploads: Vec<ClientMapBufferTileLayer>,
    pub fg_tile_layer_uploads: Vec<ClientMapBufferTileLayer>,
    pub physics_tile_layer_uploads: Vec<ClientMapBufferPhysicsTileLayer>,
    pub bg_quad_layer_uploads: Vec<ClientMapBufferQuadLayer>,
    pub fg_quad_layer_uploads: Vec<ClientMapBufferQuadLayer>,

    pub map: Map,
}

impl ClientMapBuffered {
    pub fn new(
        backend_handle: &GraphicsBackendHandle,
        buffer_object_handle: &GraphicsBufferObjectHandle,
        upload_data: ClientMapBufferUploadData,
        images: Vec<TextureContainer>,
        images_2d_array: Vec<TextureContainer2dArray>,
        sound_scene: SceneObject,
        sound_listener: SoundListener,
        sound_objects: Vec<SoundObject>,
    ) -> Self {
        fn collect_groups(
            buffer_object_handle: &GraphicsBufferObjectHandle,
            backend_handle: &GraphicsBackendHandle,
            groups: Vec<MapGroup>,
            mut tile_layer_uploads: impl Iterator<Item = ClientMapBufferTileLayer>,
            mut quad_layer_uploads: impl Iterator<Item = ClientMapBufferQuadLayer>,
            tile_render_infos: &mut Vec<MapRenderInfo>,
            quad_render_infos: &mut Vec<MapRenderInfo>,
            sound: &mut Vec<MapSoundProcessInfo>,
        ) -> Vec<MapVisualGroup> {
            groups
                .into_iter()
                .enumerate()
                .map(|(g, def)| MapVisualGroup {
                    attr: def.attr,
                    layers: def
                        .layers
                        .into_iter()
                        .enumerate()
                        .map(|(l, def)| match def {
                            MapLayer::Abritrary(def) => {
                                MapVisualLayer::Abritrary(MapVisualLayerArbitrary {
                                    buf: def,
                                    user: (),
                                })
                            }
                            MapLayer::Tile(def) => {
                                let upload_data = tile_layer_uploads.next().unwrap();
                                let render_info = upload_data.render_info;

                                let visuals = ClientMapBuffered::finish_upload_tile_layer(
                                    buffer_object_handle,
                                    backend_handle,
                                    upload_data,
                                );

                                if visuals.buffer_object_index.is_some() {
                                    tile_render_infos.push(render_info);
                                }

                                MapVisualLayer::Tile(MapVisualLayerTile {
                                    layer: def,
                                    user: visuals,
                                })
                            }
                            MapLayer::Quad(def) => {
                                let upload_data = quad_layer_uploads.next().unwrap();
                                let render_info = upload_data.3;
                                let visuals = ClientMapBuffered::finish_upload_quad_layer(
                                    buffer_object_handle,
                                    backend_handle,
                                    upload_data,
                                );

                                if visuals.buffer_object_index.is_some() {
                                    quad_render_infos.push(render_info);
                                }

                                MapVisualLayer::Quad(MapVisualLayerQuad {
                                    layer: def,
                                    user: visuals,
                                })
                            }
                            MapLayer::Sound(def) => {
                                sound.push(MapSoundProcessInfo {
                                    group_index: g,
                                    layer_index: l,
                                });

                                MapVisualLayer::Sound(MapVisualLayerSound {
                                    layer: def,
                                    user: SoundLayerSounds::default(),
                                })
                            }
                        })
                        .collect(),
                    name: def.name,
                    user: (),
                })
                .collect()
        }

        let mut bg_tile_render_infos: Vec<MapRenderInfo> = Default::default();
        let mut fg_tile_render_infos: Vec<MapRenderInfo> = Default::default();
        let mut physics_tile_render_infos: Vec<MapPhysicsRenderInfo> = Default::default();
        let mut bg_quad_render_infos: Vec<MapRenderInfo> = Default::default();
        let mut fg_quad_render_infos: Vec<MapRenderInfo> = Default::default();

        bg_tile_render_infos.reserve(upload_data.bg_tile_layer_uploads.len());
        fg_tile_render_infos.reserve(upload_data.fg_tile_layer_uploads.len());
        physics_tile_render_infos.reserve(upload_data.physics_tile_layer_uploads.len());
        bg_quad_render_infos.reserve(upload_data.bg_quad_layer_uploads.len());
        fg_quad_render_infos.reserve(upload_data.fg_quad_layer_uploads.len());

        let mut physics_tile_layer_uploads = upload_data.physics_tile_layer_uploads.into_iter();
        let bg_tile_layer_uploads = upload_data.bg_tile_layer_uploads.into_iter();
        let fg_tile_layer_uploads = upload_data.fg_tile_layer_uploads.into_iter();
        let bg_quad_layer_uploads = upload_data.bg_quad_layer_uploads.into_iter();
        let fg_quad_layer_uploads = upload_data.fg_quad_layer_uploads.into_iter();

        let mut sound = ClientMapBufferedSoundProcess::default();

        let mut res = ClientMapBuffered {
            map_visual: MapVisual {
                user: MapVisualProps {
                    sound_scene,
                    global_listener: sound_listener,
                },
                groups: MapVisualGroups {
                    physics: MapVisualPhysicsGroup {
                        attr: upload_data.map.groups.physics.attr,
                        layers: upload_data
                            .map
                            .groups
                            .physics
                            .layers
                            .into_iter()
                            .map(|def| {
                                let uploaded_data = physics_tile_layer_uploads.next().unwrap();
                                let render_info = uploaded_data.render_info.clone();
                                let visuals = Self::finish_upload_physics_tile_layer(
                                    buffer_object_handle,
                                    backend_handle,
                                    uploaded_data,
                                );

                                if visuals.base.buffer_object_index.is_some() {
                                    physics_tile_render_infos.push(render_info);
                                }
                                match def {
                                    MapLayerPhysics::Arbitrary(layer) => {
                                        MapVisualPhysicsLayer::Arbitrary(
                                            MapLayerArbitraryPhysicsSkeleton {
                                                buf: layer,
                                                user: visuals,
                                            },
                                        )
                                    }
                                    MapLayerPhysics::Game(layer) => MapVisualPhysicsLayer::Game(
                                        MapLayerTilePhysicsBaseSkeleton {
                                            def: layer,
                                            user: visuals,
                                        },
                                    ),
                                    MapLayerPhysics::Front(layer) => MapVisualPhysicsLayer::Front(
                                        MapLayerTilePhysicsBaseSkeleton {
                                            def: layer,
                                            user: visuals,
                                        },
                                    ),
                                    MapLayerPhysics::Tele(layer) => MapVisualPhysicsLayer::Tele(
                                        MapLayerTilePhysicsBaseSkeleton {
                                            def: layer,
                                            user: visuals,
                                        },
                                    ),
                                    MapLayerPhysics::Speedup(layer) => {
                                        MapVisualPhysicsLayer::Speedup(
                                            MapLayerTilePhysicsBaseSkeleton {
                                                def: layer,
                                                user: visuals,
                                            },
                                        )
                                    }
                                    MapLayerPhysics::Switch(layer) => {
                                        MapVisualPhysicsLayer::Switch(
                                            MapLayerTilePhysicsBaseSkeleton {
                                                def: layer,
                                                user: visuals,
                                            },
                                        )
                                    }
                                    MapLayerPhysics::Tune(layer) => MapVisualPhysicsLayer::Tune(
                                        MapLayerTilePhysicsBaseSkeleton {
                                            def: layer,
                                            user: visuals,
                                        },
                                    ),
                                }
                            })
                            .collect(),
                        user: (),
                    },
                    background: collect_groups(
                        buffer_object_handle,
                        backend_handle,
                        upload_data.map.groups.background,
                        bg_tile_layer_uploads,
                        bg_quad_layer_uploads,
                        &mut bg_tile_render_infos,
                        &mut bg_quad_render_infos,
                        &mut sound.background_sound_layers,
                    ),
                    foreground: collect_groups(
                        buffer_object_handle,
                        backend_handle,
                        upload_data.map.groups.foreground,
                        fg_tile_layer_uploads,
                        fg_quad_layer_uploads,
                        &mut fg_tile_render_infos,
                        &mut fg_quad_render_infos,
                        &mut sound.foreground_sound_layers,
                    ),
                    user: (),
                },
                resources: MapVisualResources {
                    images: upload_data
                        .map
                        .resources
                        .images
                        .into_iter()
                        .zip(images.into_iter())
                        .map(|(def, user)| MapVisualImage { def, user })
                        .collect(),
                    image_arrays: upload_data
                        .map
                        .resources
                        .image_arrays
                        .into_iter()
                        .zip(images_2d_array.into_iter())
                        .map(|(def, user)| MapVisualImage2dArray { def, user })
                        .collect(),
                    sounds: upload_data
                        .map
                        .resources
                        .sounds
                        .into_iter()
                        .zip(sound_objects.into_iter())
                        .map(|(def, sound_object)| MapVisualSound {
                            def,
                            user: sound_object,
                        })
                        .collect(),

                    user: (),
                },
                animations: MapVisualAnimations {
                    pos: upload_data
                        .map
                        .animations
                        .pos
                        .into_iter()
                        .map(|def| MapVisualPosAnimation { def, user: () })
                        .collect(),
                    color: upload_data
                        .map
                        .animations
                        .color
                        .into_iter()
                        .map(|def| MapVisualColorAnimation { def, user: () })
                        .collect(),
                    sound: upload_data
                        .map
                        .animations
                        .sound
                        .into_iter()
                        .map(|def| MapVisualSoundAnimation { def, user: () })
                        .collect(),

                    user: (),
                },
                config: MapVisualConfig {
                    def: upload_data.map.config,
                    user: (),
                },
                meta: MapVisualMetadata {
                    def: upload_data.map.meta,
                    user: (),
                },
            },
            render: Default::default(),
            sound,
        };

        let mut background_render_layers = [
            bg_tile_render_infos
                .into_iter()
                .map(|i| MapRenderLayer::Tile(i))
                .collect::<Vec<MapRenderLayer>>(),
            bg_quad_render_infos
                .into_iter()
                .map(|i| MapRenderLayer::Quad(i))
                .collect::<Vec<MapRenderLayer>>(),
        ]
        .concat();
        background_render_layers.sort_by(|a1, a2| {
            let a1 = a1.get_render_info();
            let a2 = a2.get_render_info();
            let a1 = a1.group_index as u128 + u128::MAX / 2 + a1.layer_index as u128;
            let a2 = a2.group_index as u128 + u128::MAX / 2 + a2.layer_index as u128;
            a1.cmp(&a2)
        });
        let mut foreground_render_layers = [
            fg_tile_render_infos
                .into_iter()
                .map(|i| MapRenderLayer::Tile(i))
                .collect::<Vec<MapRenderLayer>>(),
            fg_quad_render_infos
                .into_iter()
                .map(|i| MapRenderLayer::Quad(i))
                .collect::<Vec<MapRenderLayer>>(),
        ]
        .concat();
        foreground_render_layers.sort_by(|a1, a2| {
            let a1 = a1.get_render_info();
            let a2 = a2.get_render_info();
            let a1 = a1.group_index as u128 + u128::MAX / 2 + a1.layer_index as u128;
            let a2 = a2.group_index as u128 + u128::MAX / 2 + a2.layer_index as u128;
            a1.cmp(&a2)
        });
        let mut physics_render_layers = physics_tile_render_infos;
        physics_render_layers.sort_by(|a1, a2| {
            let a1 = a1.layer_index as u128;
            let a2 = a2.layer_index as u128;
            a1.cmp(&a2)
        });
        res.render.background_render_layers = background_render_layers;
        res.render.foreground_render_layers = foreground_render_layers;
        res.render.physics_render_layers = physics_render_layers;

        res
    }

    pub fn finish_upload_tile_layer(
        buffer_object_handle: &GraphicsBufferObjectHandle,
        backend_handle: &GraphicsBackendHandle,
        upload_data: ClientMapBufferTileLayer,
    ) -> TileLayerVisuals {
        let ClientMapBufferTileLayer {
            mem: raw_data,
            indices_count,
            visuals,
            ..
        } = upload_data;
        if raw_data.is_none() || raw_data.as_ref().unwrap().as_slice().is_empty() {
            TileLayerVisuals {
                base: visuals,
                buffer_object_index: None,
            }
        } else {
            // and finally inform the backend how many indices are required
            backend_handle.indices_num_required_notify(indices_count);
            // create the buffer object
            TileLayerVisuals {
                base: visuals,
                buffer_object_index: Some(
                    buffer_object_handle.create_buffer_object(raw_data.unwrap()),
                ),
            }
        }
    }

    pub fn finish_upload_quad_layer(
        buffer_object_handle: &GraphicsBufferObjectHandle,
        backend_handle: &GraphicsBackendHandle,
        upload_data: ClientMapBufferQuadLayer,
    ) -> QuadLayerVisuals {
        let (raw_data, _, indices_count, _) = upload_data;
        if raw_data.is_none() || raw_data.as_ref().unwrap().as_slice().is_empty() {
            QuadLayerVisuals {
                buffer_object_index: None,
            }
        } else {
            // and finally inform the backend how many indices are required
            backend_handle.indices_num_required_notify(indices_count);
            // create the buffer object
            QuadLayerVisuals {
                buffer_object_index: Some(
                    buffer_object_handle.create_buffer_object(raw_data.unwrap()),
                ),
            }
        }
    }

    pub fn finish_upload_physics_tile_layer(
        buffer_object_handle: &GraphicsBufferObjectHandle,
        backend_handle: &GraphicsBackendHandle,
        upload_data: ClientMapBufferPhysicsTileLayer,
    ) -> PhysicsTileLayerVisuals {
        let ClientMapBufferPhysicsTileLayer {
            mem,
            indices_count,
            visuals,
            render_info,
            overlays,
        } = upload_data;
        let layer_visuals = Self::finish_upload_tile_layer(
            buffer_object_handle,
            backend_handle,
            ClientMapBufferTileLayer {
                mem,
                indices_count,
                visuals,
                render_info: MapRenderInfo {
                    group_index: 0,
                    layer_index: render_info.layer_index,
                },
            },
        );

        let mut overlay_buffer_objects: Vec<PhysicsTileLayerOverlayVisuals> = Vec::new();
        for (ty, overlay_mem) in overlays {
            let visuals = Self::finish_upload_tile_layer(
                buffer_object_handle,
                backend_handle,
                ClientMapBufferTileLayer {
                    mem: overlay_mem,
                    indices_count,
                    visuals: Default::default(),
                    render_info: MapRenderInfo {
                        group_index: 0,
                        layer_index: render_info.layer_index,
                    },
                },
            );
            overlay_buffer_objects.push(PhysicsTileLayerOverlayVisuals {
                buffer_object: visuals.buffer_object_index,
                ty,
            });
        }

        PhysicsTileLayerVisuals {
            base: layer_visuals,
            overlay_buffer_objects: overlay_buffer_objects,
        }
    }

    pub fn upload_tile_layer_buffer(
        group_index: usize,
        layer_index: usize,
        layer: (
            NonZeroU16MinusOne,
            NonZeroU16MinusOne,
            bool,
            &mut dyn Iterator<Item = (u8, TileFlags, i16)>,
        ),
        is_speedup_layer: bool,
        is_game_layer: bool,
        ignore_tile_index_and_is_textured_check: bool,
        graphics_mt: &GraphicsMultiThreaded,
    ) -> Option<ClientMapBufferTileLayer> {
        let mut visuals = TileLayerVisualsBase::default();

        let (width, height, is_textured, tiles) = layer;
        let is_textured = is_textured || ignore_tile_index_and_is_textured_check;
        let width = width.get() as usize;
        let height = height.get() as usize;

        if !visuals.init(width as u32, height as u32) {
            return None;
        }
        visuals.is_textured = is_textured;

        let add_as_speedup = is_speedup_layer;

        let mut tmp_tiles: Vec<GraphicTile> = Vec::new();
        let mut tmp_border_tiles: Vec<GraphicBorderTile> = Vec::new();
        let mut tmp_border_top_tiles: Vec<GraphicBorderTile> = Vec::new();
        let mut tmp_border_left_tiles: Vec<GraphicBorderTile> = Vec::new();
        let mut tmp_border_right_tiles: Vec<GraphicBorderTile> = Vec::new();
        let mut tmp_border_bottom_tiles: Vec<GraphicBorderTile> = Vec::new();
        let mut tmp_border_corners: Vec<GraphicBorderTile> = Vec::new();

        tmp_tiles.reserve(width as usize * height as usize);
        tmp_border_tiles.reserve(width as usize * 2 + height as usize * 2 + 4);
        tmp_border_top_tiles.reserve(width as usize);
        tmp_border_bottom_tiles.reserve(width as usize);
        tmp_border_left_tiles.reserve(height as usize);
        tmp_border_right_tiles.reserve(height as usize);
        tmp_border_corners.reserve(4);

        for (i, (index, flags, angle_rotate)) in tiles.enumerate() {
            let y = i / width;
            let x = i % width;

            //the amount of tiles handled before this tile
            let tiles_handled_count = tmp_tiles.len();
            visuals.tiles_of_layer[y * width + x]
                .set_index_buffer_offset_quad(tiles_handled_count as u32);

            if add_tile(
                &mut tmp_tiles,
                index,
                flags,
                x as i32,
                y as i32,
                add_as_speedup,
                angle_rotate,
                ignore_tile_index_and_is_textured_check,
            ) {
                visuals.tiles_of_layer[y * width + x].set_drawable(true);
            }

            //do the border tiles
            if x == 0 {
                if y == 0 {
                    visuals
                        .corner_top_left
                        .set_index_buffer_offset_quad(tmp_border_corners.len() as u32);
                    if add_border_tile(
                        &mut tmp_border_corners,
                        index,
                        flags,
                        0,
                        0,
                        add_as_speedup,
                        angle_rotate,
                        &ivec2::new(-1, -1),
                        ignore_tile_index_and_is_textured_check,
                    ) {
                        visuals.corner_top_left.set_drawable(true);
                    }
                } else if y == height - 1 {
                    visuals
                        .corner_bottom_left
                        .set_index_buffer_offset_quad(tmp_border_corners.len() as u32);
                    if add_border_tile(
                        &mut tmp_border_corners,
                        index,
                        flags,
                        0,
                        0,
                        add_as_speedup,
                        angle_rotate,
                        &ivec2::new(-1, 0),
                        ignore_tile_index_and_is_textured_check,
                    ) {
                        visuals.corner_bottom_left.set_drawable(true);
                    }
                }
                visuals.border_left[y]
                    .set_index_buffer_offset_quad(tmp_border_left_tiles.len() as u32);
                if add_border_tile(
                    &mut tmp_border_left_tiles,
                    index,
                    flags,
                    0,
                    y as i32,
                    add_as_speedup,
                    angle_rotate,
                    &ivec2::new(-1, 0),
                    ignore_tile_index_and_is_textured_check,
                ) {
                    visuals.border_left[y].set_drawable(true);
                }
            } else if x == width - 1 {
                if y == 0 {
                    visuals
                        .corner_top_right
                        .set_index_buffer_offset_quad(tmp_border_corners.len() as u32);
                    if add_border_tile(
                        &mut tmp_border_corners,
                        index,
                        flags,
                        0,
                        0,
                        add_as_speedup,
                        angle_rotate,
                        &ivec2::new(0, -1),
                        ignore_tile_index_and_is_textured_check,
                    ) {
                        visuals.corner_top_right.set_drawable(true);
                    }
                } else if y == height - 1 {
                    visuals
                        .corner_bottom_right
                        .set_index_buffer_offset_quad(tmp_border_corners.len() as u32);
                    if add_border_tile(
                        &mut tmp_border_corners,
                        index,
                        flags,
                        0,
                        0,
                        add_as_speedup,
                        angle_rotate,
                        &ivec2::new(0, 0),
                        ignore_tile_index_and_is_textured_check,
                    ) {
                        visuals.corner_bottom_right.set_drawable(true);
                    }
                }
                visuals.border_right[y]
                    .set_index_buffer_offset_quad(tmp_border_right_tiles.len() as u32);
                if add_border_tile(
                    &mut tmp_border_right_tiles,
                    index,
                    flags,
                    0,
                    y as i32,
                    add_as_speedup,
                    angle_rotate,
                    &ivec2::new(0, 0),
                    ignore_tile_index_and_is_textured_check,
                ) {
                    visuals.border_right[y].set_drawable(true);
                }
            }
            if y == 0 {
                visuals.border_top[x]
                    .set_index_buffer_offset_quad(tmp_border_top_tiles.len() as u32);
                if add_border_tile(
                    &mut tmp_border_top_tiles,
                    index,
                    flags,
                    x as i32,
                    0,
                    add_as_speedup,
                    angle_rotate,
                    &ivec2::new(0, -1),
                    ignore_tile_index_and_is_textured_check,
                ) {
                    visuals.border_top[x].set_drawable(true);
                }
            } else if y == height - 1 {
                visuals.border_bottom[x]
                    .set_index_buffer_offset_quad(tmp_border_bottom_tiles.len() as u32);
                if add_border_tile(
                    &mut tmp_border_bottom_tiles,
                    index,
                    flags,
                    x as i32,
                    0,
                    add_as_speedup,
                    angle_rotate,
                    &ivec2::new(0, 0),
                    ignore_tile_index_and_is_textured_check,
                ) {
                    visuals.border_bottom[x].set_drawable(true);
                }
            }
        }

        // add the border corners, then the borders and fix their byte offsets
        let mut tiles_handled_count = tmp_border_tiles.len();
        visuals
            .corner_top_left
            .add_index_buffer_offset_quad(tiles_handled_count as u32);
        visuals
            .corner_top_right
            .add_index_buffer_offset_quad(tiles_handled_count as u32);
        visuals
            .corner_bottom_left
            .add_index_buffer_offset_quad(tiles_handled_count as u32);
        visuals
            .corner_bottom_right
            .add_index_buffer_offset_quad(tiles_handled_count as u32);
        // add the Corners to the tiles
        tmp_border_tiles.append(&mut tmp_border_corners);

        // now the borders
        tiles_handled_count = tmp_border_tiles.len();
        for i in 0..width {
            visuals.border_top[i].add_index_buffer_offset_quad(tiles_handled_count as u32);
        }
        tmp_border_tiles.append(&mut tmp_border_top_tiles);

        tiles_handled_count = tmp_border_tiles.len();
        for i in 0..width {
            visuals.border_bottom[i].add_index_buffer_offset_quad(tiles_handled_count as u32);
        }
        tmp_border_tiles.append(&mut tmp_border_bottom_tiles);

        tiles_handled_count = tmp_border_tiles.len();
        for i in 0..height {
            visuals.border_left[i].add_index_buffer_offset_quad(tiles_handled_count as u32);
        }
        tmp_border_tiles.append(&mut tmp_border_left_tiles);

        tiles_handled_count = tmp_border_tiles.len();
        for i in 0..height {
            visuals.border_right[i].add_index_buffer_offset_quad(tiles_handled_count as u32);
        }
        tmp_border_tiles.append(&mut tmp_border_right_tiles);

        // append one kill tile to the gamelayer
        if is_game_layer {
            visuals
                .border_kill_tile
                .set_index_buffer_offset_quad(tmp_border_tiles.len() as u32);
            if add_border_tile(
                &mut tmp_border_tiles,
                TileNum::Death as u8,
                TileFlags::empty(),
                0,
                0,
                false,
                -1,
                &ivec2::new(0, 0),
                ignore_tile_index_and_is_textured_check,
            ) {
                visuals.border_kill_tile.set_drawable(true);
            }
        }

        let tile_size = std::mem::size_of::<GraphicsTilePos>() * 4
            + if is_textured {
                std::mem::size_of::<GraphicsTileTex>() * 4
            } else {
                0
            };
        let border_tile_size = std::mem::size_of::<GraphicsBorderTilePos>() * 4
            + if is_textured {
                std::mem::size_of::<GraphicsBorderTileTex>() * 4
            } else {
                0
            };
        let upload_data_size =
            tmp_tiles.len() * tile_size + tmp_border_tiles.len() * border_tile_size;
        if upload_data_size > 0 {
            let indices_count = (tmp_tiles.len().max(tmp_border_tiles.len())) as u64 * 6;
            let mut upload_data_buffer =
                graphics_mt.mem_alloc(GraphicsMemoryAllocationType::Buffer {
                    required_size: upload_data_size,
                });

            let data = upload_data_buffer.as_mut_slice();
            if !tmp_tiles.is_empty() {
                let data = &mut data[..tmp_tiles.len() * tile_size];
                let size_per_tile = tmp_tiles[0].copy_into_slice(data, is_textured);
                data.par_chunks_exact_mut(size_per_tile)
                    .enumerate()
                    .for_each(|(index, data)| {
                        let tile = &tmp_tiles[index];
                        tile.copy_into_slice(data, is_textured);
                    });

                visuals.buffer_size_all_tiles = tmp_tiles.len() * tile_size;
            }
            if !tmp_border_tiles.is_empty() {
                let data = &mut data[tmp_tiles.len() * tile_size..];
                let size_per_tile = tmp_border_tiles[0].copy_into_slice(data, is_textured);
                data.par_chunks_exact_mut(size_per_tile)
                    .enumerate()
                    .for_each(|(index, data)| {
                        let tile = &tmp_border_tiles[index];
                        tile.copy_into_slice(data, is_textured);
                    });
            }
            if graphics_mt
                .try_flush_mem(&mut upload_data_buffer, false)
                .is_err()
            {
                // TODO: ignore?
            }

            Some(ClientMapBufferTileLayer {
                mem: Some(upload_data_buffer),
                indices_count,
                visuals,
                render_info: MapRenderInfo {
                    group_index,
                    layer_index,
                },
            })
        } else {
            None
        }
    }

    fn fill_tmp_quads_for_upload(quads: &[Quad]) -> Vec<TmpQuadTextured> {
        let mut tmp_quads_textured: Vec<TmpQuadTextured> = Vec::new();
        tmp_quads_textured.resize(quads.len(), Default::default());

        quads.iter().enumerate().for_each(|(i, quad)| {
            for j in 0..4 {
                let mut quad_index = j;
                if j == 2 {
                    quad_index = 3;
                } else if j == 3 {
                    quad_index = 2;
                }

                // ignore the conversion for the position coordinates
                tmp_quads_textured[i].vertices[j].x = quad.points[quad_index].x.to_num::<f32>();
                tmp_quads_textured[i].vertices[j].y = quad.points[quad_index].y.to_num::<f32>();
                tmp_quads_textured[i].vertices[j].center_x = quad.points[4].x.to_num::<f32>();
                tmp_quads_textured[i].vertices[j].center_y = quad.points[4].y.to_num::<f32>();
                tmp_quads_textured[i].vertices[j].u = quad.tex_coords[quad_index].x.to_num::<f32>();
                tmp_quads_textured[i].vertices[j].v = quad.tex_coords[quad_index].y.to_num::<f32>();
                tmp_quads_textured[i].vertices[j].r =
                    (quad.colors[quad_index].r().to_num::<f32>() * 255.0) as u8;
                tmp_quads_textured[i].vertices[j].g =
                    (quad.colors[quad_index].g().to_num::<f32>() * 255.0) as u8;
                tmp_quads_textured[i].vertices[j].b =
                    (quad.colors[quad_index].b().to_num::<f32>() * 255.0) as u8;
                tmp_quads_textured[i].vertices[j].a =
                    (quad.colors[quad_index].a().to_num::<f32>() * 255.0) as u8;
            }
        });
        tmp_quads_textured
    }

    fn upload_quad_layer_buffer(
        attr: &MapLayerQuadsAttrs,
        quads: &Vec<Quad>,
        group_index: usize,
        layer_index: usize,
        graphics_mt: &GraphicsMultiThreaded,
        ignore_is_textured_check: bool,
    ) -> Option<ClientMapBufferQuadLayer> {
        let is_textured = attr.image.is_some() || ignore_is_textured_check;

        let tmp_quads_textured = Self::fill_tmp_quads_for_upload(&quads);

        let upload_data_size = tmp_quads_textured.len() * std::mem::size_of::<TmpQuadTextured>()
            - if is_textured {
                0
            } else {
                tmp_quads_textured.len() * std::mem::size_of::<f32>() * 4 * 2
            };

        if upload_data_size > 0 {
            let mut upload_data_buffer =
                graphics_mt.mem_alloc(GraphicsMemoryAllocationType::Buffer {
                    required_size: upload_data_size,
                });

            let mut off = 0;
            tmp_quads_textured.iter().for_each(|q| {
                off += q.copy_into_slice(&mut upload_data_buffer.as_mut_slice()[off..], is_textured)
            });

            if graphics_mt
                .try_flush_mem(&mut upload_data_buffer, false)
                .is_err()
            {
                // TODO: ignore?
            }

            Some((
                Some(upload_data_buffer),
                is_textured,
                (quads.len() * 6) as u64,
                MapRenderInfo {
                    group_index,
                    layer_index,
                },
            ))
        } else {
            None
        }
    }

    pub fn upload_design_quad_layer(
        graphics_mt: &GraphicsMultiThreaded,
        attr: &MapLayerQuadsAttrs,
        quads: &Vec<Quad>,
        group_index: usize,
        layer_index: usize,
        ignore_is_textured_check: bool,
    ) -> ClientMapBufferQuadLayer {
        let mut res = ClientMapBufferQuadLayer::default();

        if let Some(data) = Self::upload_quad_layer_buffer(
            attr,
            quads,
            group_index,
            layer_index,
            graphics_mt,
            ignore_is_textured_check,
        ) {
            res = data;
        }
        res
    }

    pub fn upload_design_tile_layer(
        graphics_mt: &GraphicsMultiThreaded,
        tiles: &Vec<TileBase>,
        width: NonZeroU16MinusOne,
        height: NonZeroU16MinusOne,
        has_texture: bool,
        group_index: usize,
        layer_index: usize,
        ignore_tile_index_and_is_textured_check: bool,
    ) -> ClientMapBufferTileLayer {
        let mut res = ClientMapBufferTileLayer::default();

        let mut tiles = tiles.iter().map(|tile| (tile.index, tile.flags, -1));

        if let Some(data) = Self::upload_tile_layer_buffer(
            group_index,
            layer_index,
            (width, height, has_texture, &mut tiles),
            false,
            false,
            ignore_tile_index_and_is_textured_check,
            graphics_mt,
        ) {
            res = data;
        }
        res
    }

    pub fn upload_physics_layer(
        graphics_mt: &GraphicsMultiThreaded,
        width: NonZeroU16MinusOne,
        height: NonZeroU16MinusOne,
        tiles: MapTileLayerPhysicsTilesRef,
        layer_index: usize,
        ignore_tile_index_check: bool,
    ) -> ClientMapBufferPhysicsTileLayer {
        let mut res = ClientMapBufferPhysicsTileLayer::default();

        let mut is_switch_layer = false;
        let mut is_tele_layer = false;
        let mut is_speedup_layer = false;

        match tiles {
            MapTileLayerPhysicsTilesRef::Arbitrary(_) => {}
            MapTileLayerPhysicsTilesRef::Game(_) => {}
            MapTileLayerPhysicsTilesRef::Front(_) => {}
            MapTileLayerPhysicsTilesRef::Tele(_) => {
                is_tele_layer = true;
            }
            MapTileLayerPhysicsTilesRef::Speedup(_) => {
                is_speedup_layer = true;
            }
            MapTileLayerPhysicsTilesRef::Switch(_) => {
                is_switch_layer = true;
            }
            MapTileLayerPhysicsTilesRef::Tune(_) => {}
        }

        let mut text_overlay_count = 0;
        if is_switch_layer {
            text_overlay_count = 2;
        } else if is_tele_layer {
            text_overlay_count = 1;
        } else if is_speedup_layer {
            text_overlay_count = 2;
        }

        for cur_text_overlay in 0..text_overlay_count + 1 {
            let mut is_game_layer = false;
            let mut is_speedup_layer = false;
            let mut text_overlay_type: Option<MapRenderTextOverlayType> = None;

            let mut tiles: Box<dyn Iterator<Item = (u8, TileFlags, i16)>> = match tiles {
                MapTileLayerPhysicsTilesRef::Arbitrary(_) => Box::new([].into_iter()),
                MapTileLayerPhysicsTilesRef::Game(tiles) => {
                    is_game_layer = true;
                    Box::new(tiles.iter().map(|tile| (tile.index, tile.flags, -1)))
                }
                MapTileLayerPhysicsTilesRef::Front(tiles) => {
                    Box::new(tiles.iter().map(|tile| (tile.index, tile.flags, -1)))
                }
                MapTileLayerPhysicsTilesRef::Tele(tiles) => {
                    if cur_text_overlay == 1 {
                        text_overlay_type = Some(MapRenderTextOverlayType::Center);
                    }
                    Box::new(tiles.iter().map(|tile| {
                        let mut index = tile.tile_type;
                        let flags = TileFlags::empty();
                        if cur_text_overlay == 1 {
                            if index != TileNum::TeleCheckIn as u8
                                && index != TileNum::TeleCheckInEvil as u8
                            {
                                index = tile.number;
                            } else {
                                index = 0;
                            }
                        }

                        (index, flags, -1)
                    }))
                }
                MapTileLayerPhysicsTilesRef::Speedup(tiles) => {
                    if cur_text_overlay == 0 {
                        is_speedup_layer = true;
                    } else if cur_text_overlay == 1 {
                        text_overlay_type = Some(MapRenderTextOverlayType::Bottom);
                    } else if cur_text_overlay == 2 {
                        text_overlay_type = Some(MapRenderTextOverlayType::Top);
                    }
                    Box::new(tiles.iter().map(|tile| {
                        let mut index = tile.tile_type;
                        let flags = TileFlags::empty();
                        let angle_rotate = tile.angle;
                        if tile.force == 0 {
                            index = 0;
                        } else if cur_text_overlay == 1 {
                            index = tile.force;
                        } else if cur_text_overlay == 2 {
                            index = tile.max_speed;
                        }
                        (index, flags, angle_rotate)
                    }))
                }
                MapTileLayerPhysicsTilesRef::Switch(tiles) => {
                    if cur_text_overlay == 1 {
                        text_overlay_type = Some(MapRenderTextOverlayType::Bottom);
                    } else if cur_text_overlay == 2 {
                        text_overlay_type = Some(MapRenderTextOverlayType::Top);
                    }
                    Box::new(tiles.iter().map(|tile| {
                        let mut flags = TileFlags::empty();
                        let mut index = tile.tile_type;
                        if cur_text_overlay == 0 {
                            flags = tile.base.flags;
                            if index == TILE_SWITCHTIMEDOPEN as u8 {
                                index = 8;
                            }
                        } else if cur_text_overlay == 1 {
                            index = tile.number;
                        } else if cur_text_overlay == 2 {
                            index = tile.delay;
                        }

                        (index, flags, -1)
                    }))
                }
                MapTileLayerPhysicsTilesRef::Tune(tiles) => Box::new(
                    tiles
                        .iter()
                        .map(|tile| (tile.tile_type, tile.base.flags, -1)),
                ),
            };

            if let Some(data) = Self::upload_tile_layer_buffer(
                0,
                layer_index,
                (width, height, true, &mut tiles),
                is_speedup_layer,
                is_game_layer,
                ignore_tile_index_check,
                graphics_mt,
            ) {
                if cur_text_overlay == 0 {
                    res = ClientMapBufferPhysicsTileLayer {
                        mem: data.mem,
                        indices_count: data.indices_count,
                        visuals: data.visuals,
                        render_info: MapPhysicsRenderInfo {
                            layer_index: data.render_info.layer_index,
                        },
                        overlays: Vec::new(),
                    };
                } else {
                    res.overlays.push((text_overlay_type.unwrap(), data.mem));
                }
            }
        }
        res
    }

    /// `F` takes the amount of tiles to skip as argument
    fn update_tile_layer<'a, F>(
        tp: &Arc<rayon::ThreadPool>,
        buffer_object_index: &mut Option<BufferObject>,
        layer_width: NonZeroU16MinusOne,
        layer_height: NonZeroU16MinusOne,
        x: u16,
        y: u16,
        width: NonZeroU16MinusOne,
        height: NonZeroU16MinusOne,
        get_tiles_it: F,
        is_speedup_layer: bool,
        is_textured: bool,
    ) where
        F: Fn(usize) -> Box<dyn Iterator<Item = (u8, TileFlags, i16)> + 'a> + 'a,
    {
        let size_of_tile = if is_textured {
            std::mem::size_of::<GraphicTile>()
        } else {
            std::mem::size_of::<GraphicsTilePos>() * 4
        };
        let size_of_border_tile = if is_textured {
            std::mem::size_of::<GraphicBorderTile>()
        } else {
            std::mem::size_of::<GraphicsBorderTilePos>() * 4
        };

        let add_as_speedup = is_speedup_layer;
        let ignore_tile_index_check = true;

        // border tiles after all rows are updated
        let mut tmp_border_top_tiles: Vec<GraphicBorderTile> = Vec::new();
        let mut tmp_border_left_tiles: Vec<GraphicBorderTile> = Vec::new();
        let mut tmp_border_right_tiles: Vec<GraphicBorderTile> = Vec::new();
        let mut tmp_border_bottom_tiles: Vec<GraphicBorderTile> = Vec::new();
        let mut tmp_border_corner_top_left: Vec<GraphicBorderTile> = Vec::new();
        let mut tmp_border_corner_top_right: Vec<GraphicBorderTile> = Vec::new();
        let mut tmp_border_corner_bottom_left: Vec<GraphicBorderTile> = Vec::new();
        let mut tmp_border_corner_bottom_right: Vec<GraphicBorderTile> = Vec::new();

        let mut tmp_tiles: Vec<GraphicTile> =
            Vec::with_capacity(width.get() as usize * height.get() as usize);
        let mut tile_update_regions: Vec<CommandUpdateBufferObjectRegion> =
            Vec::with_capacity(height.get() as usize);

        for y in y..y + height.get() {
            let tile_index_skip = y as usize * layer_width.get() as usize + x as usize;
            let tiles = get_tiles_it(tile_index_skip);

            let tmp_tiles_len = tmp_tiles.len();

            let update_tiles = tiles.take(width.get() as usize);
            update_tiles
                .enumerate()
                .for_each(|(i, (index, flags, angle_rotate))| {
                    let y = (i + tile_index_skip) / layer_width.get() as usize;
                    let x = (i + tile_index_skip) % layer_width.get() as usize;

                    let width = layer_width.get() as usize;
                    let height = layer_height.get() as usize;

                    if add_tile(
                        &mut tmp_tiles,
                        index,
                        flags,
                        x as i32,
                        y as i32,
                        add_as_speedup,
                        angle_rotate,
                        ignore_tile_index_check,
                    ) {
                        // nothing to do
                    }
                    if x == 0 {
                        if y == 0 {
                            if add_border_tile(
                                &mut tmp_border_corner_top_left,
                                index,
                                flags,
                                0,
                                0,
                                add_as_speedup,
                                angle_rotate,
                                &ivec2::new(-1, -1),
                                ignore_tile_index_check,
                            ) {
                                // nothing to do
                            }
                        } else if y == height - 1 {
                            if add_border_tile(
                                &mut tmp_border_corner_bottom_left,
                                index,
                                flags,
                                0,
                                0,
                                add_as_speedup,
                                angle_rotate,
                                &ivec2::new(-1, 0),
                                ignore_tile_index_check,
                            ) {
                                // nothing to do
                            }
                        }
                        if add_border_tile(
                            &mut tmp_border_left_tiles,
                            index,
                            flags,
                            0,
                            y as i32,
                            add_as_speedup,
                            angle_rotate,
                            &ivec2::new(-1, 0),
                            ignore_tile_index_check,
                        ) {
                            // nothing to do
                        }
                    } else if x == width - 1 {
                        if y == 0 {
                            if add_border_tile(
                                &mut tmp_border_corner_top_right,
                                index,
                                flags,
                                0,
                                0,
                                add_as_speedup,
                                angle_rotate,
                                &ivec2::new(0, -1),
                                ignore_tile_index_check,
                            ) {
                                // nothing to do
                            }
                        } else if y == height - 1 {
                            if add_border_tile(
                                &mut tmp_border_corner_bottom_right,
                                index,
                                flags,
                                0,
                                0,
                                add_as_speedup,
                                angle_rotate,
                                &ivec2::new(0, 0),
                                ignore_tile_index_check,
                            ) {
                                // nothing to do
                            }
                        }
                        if add_border_tile(
                            &mut tmp_border_right_tiles,
                            index,
                            flags,
                            0,
                            y as i32,
                            add_as_speedup,
                            angle_rotate,
                            &ivec2::new(0, 0),
                            ignore_tile_index_check,
                        ) {
                            // nothing to do
                        }
                    }
                    if y == 0 {
                        if add_border_tile(
                            &mut tmp_border_top_tiles,
                            index,
                            flags,
                            x as i32,
                            0,
                            add_as_speedup,
                            angle_rotate,
                            &ivec2::new(0, -1),
                            ignore_tile_index_check,
                        ) {
                            // nothing to do
                        }
                    } else if y == height - 1 {
                        if add_border_tile(
                            &mut tmp_border_bottom_tiles,
                            index,
                            flags,
                            x as i32,
                            0,
                            add_as_speedup,
                            angle_rotate,
                            &ivec2::new(0, 0),
                            ignore_tile_index_check,
                        ) {
                            // nothing to do
                        }
                    }
                });

            if tmp_tiles.len() > tmp_tiles_len {
                tile_update_regions.push(CommandUpdateBufferObjectRegion {
                    src_offset: tmp_tiles_len * size_of_tile,
                    dst_offset: tile_index_skip * size_of_tile,
                    size: (tmp_tiles.len() - tmp_tiles_len) * size_of_tile,
                })
            }
        }
        if !tmp_tiles.is_empty() && !tile_update_regions.is_empty() {
            let mut upload_data: Vec<u8> = Default::default();
            upload_data.resize(tmp_tiles.len() * size_of_tile, Default::default());
            tp.install(|| {
                upload_data
                    .par_chunks_exact_mut(size_of_tile)
                    .enumerate()
                    .for_each(|(index, upload_data)| {
                        let tile = &tmp_tiles[index];
                        tile.copy_into_slice(upload_data, is_textured);
                    });
            });

            buffer_object_index
                .as_ref()
                .unwrap()
                .update_buffer_object(upload_data, tile_update_regions);
        }

        // do the corner tiles
        if !tmp_border_corner_top_left.is_empty() {
            let mut upload_data: Vec<u8> = Default::default();
            upload_data.resize(
                tmp_border_corner_top_left.len() * size_of_border_tile,
                Default::default(),
            );
            let mut off = 0;
            tmp_border_corner_top_left
                .iter()
                .for_each(|tile| off += tile.copy_into_slice(&mut upload_data[off..], is_textured));

            let upload_data_len = upload_data.len();
            buffer_object_index.as_ref().unwrap().update_buffer_object(
                upload_data,
                [CommandUpdateBufferObjectRegion {
                    src_offset: 0,
                    dst_offset: (layer_width.get() as usize * layer_height.get() as usize)
                        * size_of_tile,
                    size: upload_data_len,
                }]
                .into(),
            );
        }
        if !tmp_border_corner_top_right.is_empty() {
            let mut upload_data: Vec<u8> = Default::default();
            upload_data.resize(
                tmp_border_corner_top_right.len() * size_of_border_tile,
                Default::default(),
            );
            let mut off = 0;
            tmp_border_corner_top_right
                .iter()
                .for_each(|tile| off += tile.copy_into_slice(&mut upload_data[off..], is_textured));

            let upload_data_len = upload_data.len();
            buffer_object_index.as_ref().unwrap().update_buffer_object(
                upload_data,
                [CommandUpdateBufferObjectRegion {
                    src_offset: 0,
                    dst_offset: (layer_width.get() as usize * layer_height.get() as usize)
                        * size_of_tile
                        + 1 * size_of_border_tile,
                    size: upload_data_len,
                }]
                .into(),
            );
        }
        if !tmp_border_corner_bottom_left.is_empty() {
            let mut upload_data: Vec<u8> = Default::default();
            upload_data.resize(
                tmp_border_corner_bottom_left.len() * size_of_border_tile,
                Default::default(),
            );
            let mut off = 0;
            tmp_border_corner_bottom_left
                .iter()
                .for_each(|tile| off += tile.copy_into_slice(&mut upload_data[off..], is_textured));

            let upload_data_len = upload_data.len();
            buffer_object_index.as_ref().unwrap().update_buffer_object(
                upload_data,
                [CommandUpdateBufferObjectRegion {
                    src_offset: 0,
                    dst_offset: (layer_width.get() as usize * layer_height.get() as usize)
                        * size_of_tile
                        + 2 * size_of_border_tile,
                    size: upload_data_len,
                }]
                .into(),
            );
        }
        if !tmp_border_corner_bottom_right.is_empty() {
            let mut upload_data: Vec<u8> = Default::default();
            upload_data.resize(
                tmp_border_corner_bottom_right.len() * size_of_border_tile,
                Default::default(),
            );
            let mut off = 0;
            tmp_border_corner_bottom_right
                .iter()
                .for_each(|tile| off += tile.copy_into_slice(&mut upload_data[off..], is_textured));

            let upload_data_len = upload_data.len();
            buffer_object_index.as_ref().unwrap().update_buffer_object(
                upload_data,
                [CommandUpdateBufferObjectRegion {
                    src_offset: 0,
                    dst_offset: (layer_width.get() as usize
                        * layer_height.get() as usize
                        * size_of_tile)
                        + 3 * size_of_border_tile,
                    size: upload_data_len,
                }]
                .into(),
            );
        }

        // now do the border tiles
        if !tmp_border_top_tiles.is_empty() {
            let mut upload_data: Vec<u8> = Default::default();
            upload_data.resize(
                tmp_border_top_tiles.len() * size_of_border_tile,
                Default::default(),
            );
            let mut off = 0;
            tmp_border_top_tiles
                .iter()
                .for_each(|tile| off += tile.copy_into_slice(&mut upload_data[off..], is_textured));

            let upload_data_len = upload_data.len();
            buffer_object_index.as_ref().unwrap().update_buffer_object(
                upload_data,
                [CommandUpdateBufferObjectRegion {
                    src_offset: 0,
                    dst_offset: (layer_width.get() as usize * layer_height.get() as usize)
                        * size_of_tile
                        + (4 + x as usize) * size_of_border_tile,
                    size: upload_data_len,
                }]
                .into(),
            );
        }
        if !tmp_border_bottom_tiles.is_empty() {
            let mut upload_data: Vec<u8> = Default::default();
            upload_data.resize(
                tmp_border_bottom_tiles.len() * size_of_border_tile,
                Default::default(),
            );
            let mut off = 0;
            tmp_border_bottom_tiles
                .iter()
                .for_each(|tile| off += tile.copy_into_slice(&mut upload_data[off..], is_textured));

            let upload_data_len = upload_data.len();
            buffer_object_index.as_ref().unwrap().update_buffer_object(
                upload_data,
                [CommandUpdateBufferObjectRegion {
                    src_offset: 0,
                    dst_offset: (layer_width.get() as usize * layer_height.get() as usize)
                        * size_of_tile
                        + (4 + layer_width.get() as usize + x as usize) * size_of_border_tile,
                    size: upload_data_len,
                }]
                .into(),
            );
        }
        if !tmp_border_left_tiles.is_empty() {
            let mut upload_data: Vec<u8> = Default::default();
            upload_data.resize(
                tmp_border_left_tiles.len() * size_of_border_tile,
                Default::default(),
            );
            let mut off = 0;
            tmp_border_left_tiles
                .iter()
                .for_each(|tile| off += tile.copy_into_slice(&mut upload_data[off..], is_textured));

            let upload_data_len = upload_data.len();
            buffer_object_index.as_ref().unwrap().update_buffer_object(
                upload_data,
                [CommandUpdateBufferObjectRegion {
                    src_offset: 0,
                    dst_offset: (layer_width.get() as usize * layer_height.get() as usize)
                        * size_of_tile
                        + (4 + layer_width.get() as usize * 2 + y as usize) * size_of_border_tile,
                    size: upload_data_len,
                }]
                .into(),
            );
        }
        if !tmp_border_right_tiles.is_empty() {
            let mut upload_data: Vec<u8> = Default::default();
            upload_data.resize(
                tmp_border_right_tiles.len() * size_of_border_tile,
                Default::default(),
            );
            let mut off = 0;
            tmp_border_right_tiles
                .iter()
                .for_each(|tile| off += tile.copy_into_slice(&mut upload_data[off..], is_textured));

            let upload_data_len = upload_data.len();
            buffer_object_index.as_ref().unwrap().update_buffer_object(
                upload_data,
                [CommandUpdateBufferObjectRegion {
                    src_offset: 0,
                    dst_offset: (layer_width.get() as usize * layer_height.get() as usize)
                        * size_of_tile
                        + (4 + layer_width.get() as usize * 2
                            + layer_height.get() as usize
                            + y as usize)
                            * size_of_border_tile,
                    size: upload_data_len,
                }]
                .into(),
            );
        }
    }

    /// should only be called on layers that were created with `ignore_tile_index_check`
    pub fn update_physics_layer<L>(
        tp: &Arc<rayon::ThreadPool>,
        group_width: NonZeroU16MinusOne,
        group_height: NonZeroU16MinusOne,
        layer: &mut MapLayerPhysicsSkeleton<L>,
        x: u16,
        y: u16,
        width: NonZeroU16MinusOne,
        height: NonZeroU16MinusOne,
    ) where
        L: BorrowMut<PhysicsTileLayerVisuals>,
    {
        let mut is_switch_layer = false;
        let mut is_tele_layer = false;
        let mut is_speedup_layer = false;

        match &layer {
            MapLayerPhysicsSkeleton::Arbitrary(_) => {}
            MapLayerPhysicsSkeleton::Game(_) => {}
            MapLayerPhysicsSkeleton::Front(_) => {}
            MapLayerPhysicsSkeleton::Tele(_) => {
                is_tele_layer = true;
            }
            MapLayerPhysicsSkeleton::Speedup(_) => {
                is_speedup_layer = true;
            }
            MapLayerPhysicsSkeleton::Switch(_) => {
                is_switch_layer = true;
            }
            MapLayerPhysicsSkeleton::Tune(_) => {}
        }

        let mut text_overlay_count = 0;
        if is_switch_layer {
            text_overlay_count = 2;
        } else if is_tele_layer {
            text_overlay_count = 1;
        } else if is_speedup_layer {
            text_overlay_count = 2;
        }

        for cur_text_overlay in 0..text_overlay_count + 1 {
            let is_speedup_layer =
                cur_text_overlay == 0 && matches!(layer, MapLayerPhysicsSkeleton::Speedup(_));
            let mut buffer_object = if cur_text_overlay == 0 {
                &layer.user_mut().borrow_mut().base.buffer_object_index
            } else {
                &layer.user_mut().borrow_mut().overlay_buffer_objects[cur_text_overlay - 1]
                    .buffer_object
            }
            .clone();
            Self::update_tile_layer(
                tp,
                &mut buffer_object,
                group_width,
                group_height,
                x,
                y,
                width,
                height,
                |skip| match &layer {
                    MapLayerPhysicsSkeleton::Arbitrary(_) => Box::new([].into_iter()),
                    MapLayerPhysicsSkeleton::Game(layer) => Box::new(
                        layer.def.tiles[skip..]
                            .iter()
                            .map(|tile| (tile.index, tile.flags, -1)),
                    ),
                    MapLayerPhysicsSkeleton::Front(layer) => Box::new(
                        layer.def.tiles[skip..]
                            .iter()
                            .map(|tile| (tile.index, tile.flags, -1)),
                    ),
                    MapLayerPhysicsSkeleton::Tele(layer) => {
                        Box::new(layer.def.tiles[skip..].iter().map(|tile| {
                            let mut index = tile.tile_type;
                            let flags = TileFlags::empty();
                            if cur_text_overlay == 1 {
                                if index != TileNum::TeleCheckIn as u8
                                    && index != TileNum::TeleCheckInEvil as u8
                                {
                                    index = tile.number;
                                } else {
                                    index = 0;
                                }
                            }

                            (index, flags, -1)
                        }))
                    }
                    MapLayerPhysicsSkeleton::Speedup(layer) => {
                        Box::new(layer.def.tiles[skip..].iter().map(|tile| {
                            let mut index = tile.tile_type;
                            let flags = TileFlags::empty();
                            let angle_rotate = tile.angle;
                            if tile.force == 0 {
                                index = 0;
                            } else if cur_text_overlay == 1 {
                                index = tile.force;
                            } else if cur_text_overlay == 2 {
                                index = tile.max_speed;
                            }
                            (index, flags, angle_rotate)
                        }))
                    }
                    MapLayerPhysicsSkeleton::Switch(layer) => {
                        Box::new(layer.def.tiles[skip..].iter().map(|tile| {
                            let mut flags = TileFlags::empty();
                            let mut index = tile.tile_type;
                            if cur_text_overlay == 0 {
                                flags = tile.base.flags;
                                if index == TILE_SWITCHTIMEDOPEN as u8 {
                                    index = 8;
                                }
                            } else if cur_text_overlay == 1 {
                                index = tile.number;
                            } else if cur_text_overlay == 2 {
                                index = tile.delay;
                            }

                            (index, flags, -1)
                        }))
                    }
                    MapLayerPhysicsSkeleton::Tune(layer) => Box::new(
                        layer.def.tiles[skip..]
                            .iter()
                            .map(|tile| (tile.tile_type, tile.base.flags, -1)),
                    ),
                },
                is_speedup_layer,
                true,
            );
            *if cur_text_overlay == 0 {
                &mut layer.user_mut().borrow_mut().base.buffer_object_index
            } else {
                &mut layer.user_mut().borrow_mut().overlay_buffer_objects[cur_text_overlay - 1]
                    .buffer_object
            } = buffer_object;
        }
    }

    /// should only be called on layers that were created with `ignore_tile_index_and_is_textured_check`
    pub fn update_design_tile_layer<T>(
        tp: &Arc<rayon::ThreadPool>,
        layer: &mut MapLayerTileSkeleton<T>,
        x: u16,
        y: u16,
        width: NonZeroU16MinusOne,
        height: NonZeroU16MinusOne,
    ) where
        T: BorrowMut<TileLayerVisuals>,
    {
        let layer_width = layer.layer.attr.width;
        let layer_height = layer.layer.attr.height;

        Self::update_tile_layer(
            tp,
            &mut layer.user.borrow_mut().buffer_object_index,
            layer_width,
            layer_height,
            x,
            y,
            width,
            height,
            |skip| {
                Box::new(
                    layer.layer.tiles[skip..]
                        .iter()
                        .map(|tile| (tile.index, tile.flags, -1)),
                )
            },
            false,
            true, // always true bcs `ignore_tile_index_and_is_textured_check`
        )
    }

    pub fn update_design_quad_layer<Q>(
        layer: &mut MapLayerQuadSkeleton<Q>,
        update_range: Range<usize>,
    ) where
        Q: BorrowMut<QuadLayerVisuals>,
    {
        // for quad layers that are update, always assume they are textured
        let is_textured = true;

        let tmp_quads_textured =
            Self::fill_tmp_quads_for_upload(&layer.layer.quads[update_range.clone()]);

        let single_quad_size = std::mem::size_of::<TmpQuadTextured>()
            - if is_textured {
                0
            } else {
                tmp_quads_textured.len() * std::mem::size_of::<f32>() * 4 * 2
            };
        let upload_data_size = tmp_quads_textured.len() * single_quad_size;

        let mut upload_data_buffer = Vec::with_capacity(upload_data_size);
        upload_data_buffer.resize(upload_data_size, Default::default());

        let mut off = 0;
        tmp_quads_textured.iter().for_each(|q| {
            off += q.copy_into_slice(&mut upload_data_buffer.as_mut_slice()[off..], is_textured);
        });

        let upload_data_len = upload_data_buffer.len();
        layer
            .user
            .borrow_mut()
            .buffer_object_index
            .as_ref()
            .unwrap()
            .update_buffer_object(
                upload_data_buffer,
                [CommandUpdateBufferObjectRegion {
                    src_offset: 0,
                    dst_offset: update_range.start * single_quad_size,
                    size: upload_data_len,
                }]
                .into(),
            );
    }

    pub fn prepare_upload(
        graphics_mt: &GraphicsMultiThreaded,
        map: Map,
    ) -> ClientMapBufferUploadData {
        //prepare all visuals for all tile layers
        struct TileLayerProps {
            group_index: usize,
            layer_index: usize,
        }
        type QuadLayerProps = (usize, usize);
        let mut bg_tile_layers: Vec<TileLayerProps> = Vec::new();
        let mut fg_tile_layers: Vec<TileLayerProps> = Vec::new();
        let mut bg_quad_layers: Vec<QuadLayerProps> = Vec::new();
        let mut fg_quad_layers: Vec<QuadLayerProps> = Vec::new();

        let fill_groups = |groups: &Vec<MapGroup>,
                           tile_layers: &mut Vec<TileLayerProps>,
                           quad_layers: &mut Vec<QuadLayerProps>| {
            for (g, group) in groups.iter().enumerate() {
                for (l, layer) in group.layers.iter().enumerate() {
                    match layer {
                        MapLayer::Tile(_) => {
                            tile_layers.push(TileLayerProps {
                                group_index: g,
                                layer_index: l,
                            });
                        }
                        MapLayer::Quad(_q_layer) => {
                            quad_layers.push((g, l));
                        }
                        _ => {
                            // ignore
                        }
                    }
                }
            }
        };
        fill_groups(
            &map.groups.background,
            &mut bg_tile_layers,
            &mut bg_quad_layers,
        );
        fill_groups(
            &map.groups.foreground,
            &mut fg_tile_layers,
            &mut fg_quad_layers,
        );

        let bg_tile_layer_uploads: Vec<ClientMapBufferTileLayer> = bg_tile_layers
            .par_iter()
            .map(
                |&TileLayerProps {
                     group_index,
                     layer_index,
                 }| {
                    let group = &map.groups.background[group_index];
                    let layer = &group.layers[layer_index];

                    if let MapLayer::Tile(layer) = layer {
                        Self::upload_design_tile_layer(
                            graphics_mt,
                            &layer.tiles,
                            layer.attr.width,
                            layer.attr.height,
                            layer.attr.image_array.is_some(),
                            group_index,
                            layer_index,
                            false,
                        )
                    } else {
                        panic!("this should not happen")
                    }
                },
            )
            .collect();

        let fg_tile_layer_uploads: Vec<ClientMapBufferTileLayer> = fg_tile_layers
            .par_iter()
            .map(
                |&TileLayerProps {
                     group_index,
                     layer_index,
                 }| {
                    let group = &map.groups.foreground[group_index];
                    let layer = &group.layers[layer_index];

                    if let MapLayer::Tile(layer) = layer {
                        Self::upload_design_tile_layer(
                            graphics_mt,
                            &layer.tiles,
                            layer.attr.width,
                            layer.attr.height,
                            layer.attr.image_array.is_some(),
                            group_index,
                            layer_index,
                            false,
                        )
                    } else {
                        panic!("this should not happen")
                    }
                },
            )
            .collect();

        let physics_tile_layer_uploads: Vec<ClientMapBufferPhysicsTileLayer> = map
            .groups
            .physics
            .layers
            .par_iter()
            .enumerate()
            .map(|(layer_index, _)| {
                let group = &map.groups.physics;
                let layer = &group.layers[layer_index];

                Self::upload_physics_layer(
                    graphics_mt,
                    group.attr.width,
                    group.attr.height,
                    layer.as_ref().tiles_ref(),
                    layer_index,
                    false,
                )
            })
            .collect();

        let bg_quad_layer_uploads: Vec<ClientMapBufferQuadLayer> = bg_quad_layers
            .par_iter()
            .map(|&(group_index, layer_index)| {
                let group = &map.groups.background[group_index];
                let layer = &group.layers[layer_index];
                if let MapLayer::Quad(layer) = layer {
                    Self::upload_design_quad_layer(
                        graphics_mt,
                        &layer.attr,
                        &layer.quads,
                        group_index,
                        layer_index,
                        false,
                    )
                } else {
                    panic!("this should not happen.")
                }
            })
            .collect();

        let fg_quad_layer_uploads: Vec<ClientMapBufferQuadLayer> = fg_quad_layers
            .par_iter()
            .map(|&(group_index, layer_index)| {
                let group = &map.groups.foreground[group_index];
                let layer = &group.layers[layer_index];

                if let MapLayer::Quad(layer) = layer {
                    Self::upload_design_quad_layer(
                        graphics_mt,
                        &layer.attr,
                        &layer.quads,
                        group_index,
                        layer_index,
                        false,
                    )
                } else {
                    panic!("this should not happen.")
                }
            })
            .collect();

        ClientMapBufferUploadData {
            bg_tile_layer_uploads,
            fg_tile_layer_uploads,
            physics_tile_layer_uploads,
            bg_quad_layer_uploads,
            fg_quad_layer_uploads,
            map,
        }
    }
}
