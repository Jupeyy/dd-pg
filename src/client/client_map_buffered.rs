use std::ops::{Index, IndexMut};

use rayon::{
    prelude::{
        IndexedParallelIterator, IntoParallelRefMutIterator, ParallelDrainRange, ParallelIterator,
    },
    slice::ParallelSliceMut,
};

use shared_base::{
    datafile::CDatafileWrapper,
    mapdef::{
        CMapItemGroup, MapLayer, MapTileLayerDetail, TileFlag, TileNum, TILE_SWITCHTIMEDOPEN,
    },
};

use math::math::{
    fx2f,
    vector::{vec2, vec3},
    PI,
};

use graphics::{
    graphics::{
        Graphics, GraphicsBufferContainerHandleInterface, GraphicsBufferObjectHandleInterface,
    },
    graphics_mt::GraphicsMultiThreaded,
};

use graphics_types::{
    command_buffer::{BufferContainerIndex, GraphicsType, SAttribute, SBufferContainerInfo},
    types::{GraphicsBackendMemory, GraphicsMemoryAllocationType},
};

#[derive(Copy, Clone, Default)]
pub struct STileVisual {
    index_buffer_byte_offset: u32,
}

impl STileVisual {
    pub fn do_draw(&self) -> bool {
        return (self.index_buffer_byte_offset & 0x00000001) != 0;
    }

    pub fn draw(&mut self, set_draw: bool) {
        self.index_buffer_byte_offset =
            (if set_draw { 0x00000001 } else { 0 }) | (self.index_buffer_byte_offset & 0xFFFFFFFE);
    }

    pub fn index_buffer_byte_offset(&self) -> usize {
        return (self.index_buffer_byte_offset & 0xFFFFFFFE) as usize;
    }

    pub fn set_index_buffer_offset(&mut self, index_buffer_byte_off: u32) {
        self.index_buffer_byte_offset =
            index_buffer_byte_off | (self.index_buffer_byte_offset & 0x00000001);
    }

    pub fn add_index_buffer_byte_offset(&mut self, index_buffer_byte_off: u32) {
        self.index_buffer_byte_offset = ((self.index_buffer_byte_offset & 0xFFFFFFFE)
            + index_buffer_byte_off)
            | (self.index_buffer_byte_offset & 0x00000001);
    }
}

#[derive(Default)]
pub struct STileLayerVisualsBase {
    pub tiles_of_layer: Vec<STileVisual>,

    pub border_top_left: STileVisual,
    pub border_top_right: STileVisual,
    pub border_bottom_right: STileVisual,
    pub border_bottom_left: STileVisual,

    pub border_kill_tile: STileVisual, //end of map kill tile -- game layer only

    pub border_top: Vec<STileVisual>,
    pub border_left: Vec<STileVisual>,
    pub border_right: Vec<STileVisual>,
    pub border_bottom: Vec<STileVisual>,

    pub width: u32,
    pub height: u32,
    pub is_textured: bool,
}

impl STileLayerVisualsBase {
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
            .resize(height as usize * width as usize, STileVisual::default());

        if width > 2 {
            self.border_top
                .resize(width as usize - 2, STileVisual::default());
            self.border_bottom
                .resize(width as usize - 2, STileVisual::default());
        }
        if height > 2 {
            self.border_left
                .resize(height as usize - 2, STileVisual::default());
            self.border_right
                .resize(height as usize - 2, STileVisual::default());
        }
        return true;
    }
}

pub struct STileLayerVisuals {
    pub base: STileLayerVisualsBase,
    pub buffer_container_index: Option<BufferContainerIndex>,
}

#[derive(Copy, Clone, Default)]
pub struct SQuadVisual {
    pub index_buffer_byte_offset: usize,
}

#[derive(Default)]
pub struct SQuadLayerVisualsBase {
    pub quad_num: usize,
    pub quads_of_layer: Vec<SQuadVisual>,

    pub is_textured: bool,
}

impl SQuadLayerVisualsBase {
    pub fn new() -> Self {
        Default::default()
    }
}

pub struct SQuadLayerVisuals {
    pub base: SQuadLayerVisualsBase,

    pub buffer_container_index: Option<BufferContainerIndex>,
}

trait UploadDataAsBytes {
    fn copy_into_slice(&self, dest: &mut [u8], index: usize);
}

#[repr(C)]
#[derive(Default)]
struct SGraphicTile {
    top_left: vec2,
    top_right: vec2,
    bottom_right: vec2,
    bottom_left: vec2,
}

impl Index<usize> for SGraphicTile {
    type Output = vec2;

    fn index(&self, index: usize) -> &vec2 {
        match index {
            0 => &self.top_left,
            1 => &self.top_right,
            2 => &self.bottom_right,
            3 => &self.bottom_left,
            _ => panic!("index out of bounds"),
        }
    }
}

impl UploadDataAsBytes for SGraphicTile {
    fn copy_into_slice(&self, dest: &mut [u8], index: usize) {
        let mut off: usize = 0;
        match index {
            0 => {
                self.top_left.x.to_ne_bytes().iter().for_each(|byte| {
                    dest[off] = *byte;
                    off += 1;
                });
                self.top_left.y.to_ne_bytes().iter().for_each(|byte| {
                    dest[off] = *byte;
                    off += 1;
                });
            }
            1 => {
                self.top_right.x.to_ne_bytes().iter().for_each(|byte| {
                    dest[off] = *byte;
                    off += 1;
                });
                self.top_right.y.to_ne_bytes().iter().for_each(|byte| {
                    dest[off] = *byte;
                    off += 1;
                });
            }
            2 => {
                self.bottom_right.x.to_ne_bytes().iter().for_each(|byte| {
                    dest[off] = *byte;
                    off += 1;
                });
                self.bottom_right.y.to_ne_bytes().iter().for_each(|byte| {
                    dest[off] = *byte;
                    off += 1;
                });
            }
            3 => {
                self.bottom_left.x.to_ne_bytes().iter().for_each(|byte| {
                    dest[off] = *byte;
                    off += 1;
                });
                self.bottom_left.y.to_ne_bytes().iter().for_each(|byte| {
                    dest[off] = *byte;
                    off += 1;
                });
            }
            _ => panic!("out of bounds"),
        }
    }
}

impl IndexMut<usize> for SGraphicTile {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        match index {
            0 => &mut self.top_left,
            1 => &mut self.top_right,
            2 => &mut self.bottom_right,
            3 => &mut self.bottom_left,
            _ => panic!("index out of bounds"),
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Default)]
struct SGraphicTileTexureCoords {
    tex_coord_top_left: vec3,
    tex_coord_top_right: vec3,
    tex_coord_bottom_right: vec3,
    tex_coord_bottom_left: vec3,
}

impl UploadDataAsBytes for SGraphicTileTexureCoords {
    fn copy_into_slice(&self, dest: &mut [u8], index: usize) {
        let mut off: usize = 0;
        match index {
            0 => {
                self.tex_coord_top_left
                    .x
                    .to_ne_bytes()
                    .iter()
                    .for_each(|byte| {
                        dest[off] = *byte;
                        off += 1;
                    });
                self.tex_coord_top_left
                    .y
                    .to_ne_bytes()
                    .iter()
                    .for_each(|byte| {
                        dest[off] = *byte;
                        off += 1;
                    });
                self.tex_coord_top_left
                    .z
                    .to_ne_bytes()
                    .iter()
                    .for_each(|byte| {
                        dest[off] = *byte;
                        off += 1;
                    });
            }
            1 => {
                self.tex_coord_top_right
                    .x
                    .to_ne_bytes()
                    .iter()
                    .for_each(|byte| {
                        dest[off] = *byte;
                        off += 1;
                    });
                self.tex_coord_top_right
                    .y
                    .to_ne_bytes()
                    .iter()
                    .for_each(|byte| {
                        dest[off] = *byte;
                        off += 1;
                    });
                self.tex_coord_top_right
                    .z
                    .to_ne_bytes()
                    .iter()
                    .for_each(|byte| {
                        dest[off] = *byte;
                        off += 1;
                    });
            }
            2 => {
                self.tex_coord_bottom_right
                    .x
                    .to_ne_bytes()
                    .iter()
                    .for_each(|byte| {
                        dest[off] = *byte;
                        off += 1;
                    });
                self.tex_coord_bottom_right
                    .y
                    .to_ne_bytes()
                    .iter()
                    .for_each(|byte| {
                        dest[off] = *byte;
                        off += 1;
                    });
                self.tex_coord_bottom_right
                    .z
                    .to_ne_bytes()
                    .iter()
                    .for_each(|byte| {
                        dest[off] = *byte;
                        off += 1;
                    });
            }
            3 => {
                self.tex_coord_bottom_left
                    .x
                    .to_ne_bytes()
                    .iter()
                    .for_each(|byte| {
                        dest[off] = *byte;
                        off += 1;
                    });
                self.tex_coord_bottom_left
                    .y
                    .to_ne_bytes()
                    .iter()
                    .for_each(|byte| {
                        dest[off] = *byte;
                        off += 1;
                    });
                self.tex_coord_bottom_left
                    .z
                    .to_ne_bytes()
                    .iter()
                    .for_each(|byte| {
                        dest[off] = *byte;
                        off += 1;
                    });
            }
            _ => panic!("out of bounds"),
        }
    }
}

fn fill_tmp_tile_speedup(
    tmp_tile: &mut SGraphicTile,
    tmp_tex: Option<&mut SGraphicTileTexureCoords>,
    _flags: u8,
    index: u8,
    x: i32,
    y: i32,
    scale: i32,
    _group: &CMapItemGroup,
    angle_rotate: i16,
) {
    if let Some(tmp_tex) = tmp_tex {
        let x0: u8 = 0;
        let y0: u8 = 0;
        let x1: u8 = x0 + 1;
        let y1: u8 = y0;
        let x2: u8 = x0 + 1;
        let y2: u8 = y0 + 1;
        let x3: u8 = x0;
        let y3: u8 = y0 + 1;

        tmp_tex.tex_coord_top_left.x = x0 as f32;
        tmp_tex.tex_coord_top_left.y = y0 as f32;
        tmp_tex.tex_coord_bottom_left.x = x3 as f32;
        tmp_tex.tex_coord_bottom_left.y = y3 as f32;
        tmp_tex.tex_coord_top_right.x = x1 as f32;
        tmp_tex.tex_coord_top_right.y = y1 as f32;
        tmp_tex.tex_coord_bottom_right.x = x2 as f32;
        tmp_tex.tex_coord_bottom_right.y = y2 as f32;

        tmp_tex.tex_coord_top_left.z = index as f32;
        tmp_tex.tex_coord_bottom_left.z = index as f32;
        tmp_tex.tex_coord_top_right.z = index as f32;
        tmp_tex.tex_coord_bottom_right.z = index as f32;
    }

    //same as in rotate from Graphics()
    let angle = angle_rotate as f32 * (PI / 180.0);
    let c = angle.cos();
    let s = angle.sin();
    let mut x_r;
    let mut y_r;

    let scale_smaller = 2;
    tmp_tile.top_left.x = (x * scale + scale_smaller) as f32;
    tmp_tile.top_left.y = (y * scale + scale_smaller) as f32;
    tmp_tile.bottom_left.x = (x * scale + scale_smaller) as f32;
    tmp_tile.bottom_left.y = (y * scale + scale - scale_smaller) as f32;
    tmp_tile.top_right.x = (x * scale + scale - scale_smaller) as f32;
    tmp_tile.top_right.y = (y * scale + scale_smaller) as f32;
    tmp_tile.bottom_right.x = (x * scale + scale - scale_smaller) as f32;
    tmp_tile.bottom_right.y = (y * scale + scale - scale_smaller) as f32;

    let mut center = vec2::default();
    center.x = tmp_tile.top_left.x + (scale - scale_smaller) as f32 / 2.0;
    center.y = tmp_tile.top_left.y + (scale - scale_smaller) as f32 / 2.0;

    for i in 0 as usize..4 {
        let mut tile_vert = tmp_tile[i];
        x_r = tile_vert.x - center.x;
        y_r = tile_vert.y - center.y;
        tile_vert.x = x_r * c - y_r * s + center.x;
        tile_vert.y = x_r * s + y_r * c + center.y;
    }
}

fn fill_tmp_tile(
    tmp_tile: &mut SGraphicTile,
    tmp_tex: Option<&mut SGraphicTileTexureCoords>,
    flags: u8,
    index: u8,
    x: i32,
    y: i32,
    scale: i32,
    _group: &CMapItemGroup,
) {
    if let Some(tmp_tex) = tmp_tex {
        let mut x0: u8 = 0;
        let mut y0: u8 = 0;
        let mut x1: u8 = x0 + 1;
        let mut y1: u8 = y0;
        let mut x2: u8 = x0 + 1;
        let mut y2: u8 = y0 + 1;
        let mut x3: u8 = x0;
        let mut y3: u8 = y0 + 1;

        if (flags & TileFlag::XFLIP as u8) != 0 {
            x0 = x2;
            x1 = x3;
            x2 = x3;
            x3 = x0;
        }

        if (flags & TileFlag::YFLIP as u8) != 0 {
            y0 = y3;
            y2 = y1;
            y3 = y1;
            y1 = y0;
        }

        if (flags & TileFlag::ROTATE as u8) != 0 {
            let mut tmp = x0;
            x0 = x3;
            x3 = x2;
            x2 = x1;
            x1 = tmp;
            tmp = y0;
            y0 = y3;
            y3 = y2;
            y2 = y1;
            y1 = tmp;
        }

        tmp_tex.tex_coord_top_left.x = x0 as f32;
        tmp_tex.tex_coord_top_left.y = y0 as f32;
        tmp_tex.tex_coord_bottom_left.x = x3 as f32;
        tmp_tex.tex_coord_bottom_left.y = y3 as f32;
        tmp_tex.tex_coord_top_right.x = x1 as f32;
        tmp_tex.tex_coord_top_right.y = y1 as f32;
        tmp_tex.tex_coord_bottom_right.x = x2 as f32;
        tmp_tex.tex_coord_bottom_right.y = y2 as f32;

        tmp_tex.tex_coord_top_left.z = index as f32;
        tmp_tex.tex_coord_bottom_left.z = index as f32;
        tmp_tex.tex_coord_top_right.z = index as f32;
        tmp_tex.tex_coord_bottom_right.z = index as f32;
    }

    tmp_tile.top_left.x = (x * scale) as f32;
    tmp_tile.top_left.y = (y * scale) as f32;
    tmp_tile.bottom_left.x = (x * scale) as f32;
    tmp_tile.bottom_left.y = (y * scale + scale) as f32;
    tmp_tile.top_right.x = (x * scale + scale) as f32;
    tmp_tile.top_right.y = (y * scale) as f32;
    tmp_tile.bottom_right.x = (x * scale + scale) as f32;
    tmp_tile.bottom_right.y = (y * scale + scale) as f32;
}

fn add_tile(
    tmp_tiles: &mut Vec<SGraphicTile>,
    tmp_tile_tex_coords: &mut Vec<SGraphicTileTexureCoords>,
    index: u8,
    flags: u8,
    x: i32,
    y: i32,
    group: &CMapItemGroup,
    do_texture_coords: bool,
    fill_speedup: bool,
    angle_rotate: i16,
) -> bool {
    if index > 0 {
        tmp_tiles.push(SGraphicTile::default());
        let tile = tmp_tiles.last_mut().unwrap();
        let mut tile_tex: Option<&mut SGraphicTileTexureCoords> = None;
        if do_texture_coords {
            tmp_tile_tex_coords.push(SGraphicTileTexureCoords::default());
            let t_tex = tmp_tile_tex_coords.last_mut().unwrap();
            tile_tex = Some(t_tex);
        }
        if fill_speedup {
            fill_tmp_tile_speedup(
                tile,
                tile_tex,
                flags,
                0,
                x,
                y,
                32,
                group,
                angle_rotate as i16,
            );
        } else {
            fill_tmp_tile(tile, tile_tex, flags, index, x, y, 32, group);
        }

        return true;
    }
    return false;
}

#[repr(C)]
#[derive(Clone, Default)]
struct STmpQuadVertexTextured {
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

impl UploadDataAsBytes for STmpQuadVertexTextured {
    fn copy_into_slice(&self, dest: &mut [u8], _index: usize /* ignore */) {
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
        self.u.to_ne_bytes().iter().for_each(|byte| {
            dest[off] = *byte;
            off += 1;
        });
        self.v.to_ne_bytes().iter().for_each(|byte| {
            dest[off] = *byte;
            off += 1;
        });
    }
}

#[repr(C)]
#[derive(Clone, Default)]
struct STmpQuadVertex {
    x: f32,
    y: f32,
    center_x: f32,
    center_y: f32,
    r: u8,
    g: u8,
    b: u8,
    a: u8,
}

impl UploadDataAsBytes for STmpQuadVertex {
    fn copy_into_slice(&self, dest: &mut [u8], _index: usize /* ignore */) {
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
    }
}

#[repr(C)]
#[derive(Clone, Default)]
struct STmpQuad {
    vertices: [STmpQuadVertex; 4],
}

impl UploadDataAsBytes for STmpQuad {
    fn copy_into_slice(&self, dest: &mut [u8], index: usize /* ignore */) {
        let mut off: usize = 0;
        self.vertices.iter().for_each(|v| {
            v.copy_into_slice(dest.split_at_mut(off).1, index);
            off += std::mem::size_of::<STmpQuadVertex>()
        });
    }
}

#[repr(C)]
#[derive(Clone, Default)]
struct STmpQuadTextured {
    vertices: [STmpQuadVertexTextured; 4],
}

impl UploadDataAsBytes for STmpQuadTextured {
    fn copy_into_slice(&self, dest: &mut [u8], index: usize /* ignore */) {
        let mut off: usize = 0;
        self.vertices.iter().for_each(|v| {
            v.copy_into_slice(dest.split_at_mut(off).1, index);
            off += std::mem::size_of::<STmpQuadVertexTextured>()
        });
    }
}

fn mem_copy_special<T: UploadDataAsBytes + Send + Sync>(
    dest: &mut [u8],
    src: &Vec<T>,
    size_single_element: usize,
    _count: usize,
    steps: usize,
) {
    // use chunks not chunks_exact, because the last element might be smaller than the additional steps size
    dest.par_chunks_mut(size_single_element + steps)
        .enumerate()
        .for_each(|(i, dst)| {
            (src[i / 4]).copy_into_slice(dst, i % 4);
        });
}

#[derive(Default)]
pub struct ClientMapBuffered {
    pub tile_layer_visuals: Vec<STileLayerVisuals>,
    pub quad_layer_visuals: Vec<SQuadLayerVisuals>,
}

#[derive(Default)]
pub struct ClientMapBufferUploadData {
    pub tile_layer_uploads: Vec<(
        usize,
        GraphicsBackendMemory,
        bool,
        usize,
        STileLayerVisualsBase,
    )>,
    pub quad_layer_uploads: Vec<(
        usize,
        GraphicsBackendMemory,
        bool,
        usize,
        SQuadLayerVisualsBase,
    )>,
}

impl ClientMapBuffered {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn map_destroy(mut self, graphics: &mut Graphics) {
        // clear everything and destroy all buffers
        for tile_layer_visual in self.tile_layer_visuals.drain(..) {
            if let Some(buffer_container_index) = tile_layer_visual.buffer_container_index {
                graphics.delete_buffer_container(buffer_container_index, true);
            }
        }
        for quad_layer_visual in self.quad_layer_visuals.drain(..) {
            if let Some(buffer_container_index) = quad_layer_visual.buffer_container_index {
                graphics.delete_buffer_container(buffer_container_index, true);
            }
        }
    }

    pub fn prepare_upload(
        graphics_mt: &GraphicsMultiThreaded,
        map: &CDatafileWrapper,
        background_only: bool,
    ) -> ClientMapBufferUploadData {
        let mut upload_data = ClientMapBufferUploadData::default();

        let mut tile_layer_visuals: Vec<STileLayerVisualsBase> = Vec::new();
        let mut quad_layer_visuals: Vec<SQuadLayerVisualsBase> = Vec::new();

        let mut passed_game_layer = false;

        //prepare all visuals for all tile layers
        let mut tile_layers: Vec<(usize, usize, usize, i32)> = Vec::new();
        let mut quad_layers: Vec<(usize, usize, usize)> = Vec::new();

        for g in 0..map.num_groups() as usize {
            let group = map.get_group(g);

            for l in 0..group.num_layers as usize {
                let layer_index = group.start_layer as usize + l;
                let layer = map.get_layer(layer_index);
                let _is_front_layer = false;
                let mut is_switch_layer = false;
                let mut is_tele_layer = false;
                let mut is_speedup_layer = false;
                let _is_tune_layer = false;
                let mut is_game_layer = false;
                let mut is_entity_layer = false;

                if map.is_game_layer(layer_index) {
                    is_game_layer = true;
                    is_entity_layer = true;
                    passed_game_layer = true;
                }

                if map.is_switch_layer(layer_index) {
                    is_entity_layer = true;
                    is_switch_layer = true;
                }

                if map.is_tele_layer(layer_index) {
                    is_entity_layer = true;
                    is_tele_layer = true;
                }

                if map.is_speedup_layer(layer_index) {
                    is_entity_layer = true;
                    is_speedup_layer = true;
                }

                if background_only {
                    if passed_game_layer {
                        return upload_data;
                    }
                }

                if let MapLayer::Tile(_) = layer {
                    let mut overlay_count = 0;
                    if is_switch_layer {
                        overlay_count = 2;
                    } else if is_tele_layer {
                        overlay_count = 1;
                    } else if is_speedup_layer {
                        overlay_count = 2;
                    }

                    let mut cur_overlay = 0;
                    while cur_overlay < overlay_count + 1 {
                        // We can later just count the tile layers to get the idx in the vector
                        let tile_layer_visuals = &mut tile_layer_visuals;
                        let visual_index = tile_layer_visuals.len();
                        tile_layer_visuals.push(STileLayerVisualsBase::default());

                        tile_layers.push((visual_index, g, layer_index, cur_overlay));

                        cur_overlay += 1;
                    }
                } else if let MapLayer::Quads(_q_layer) = layer {
                    let quad_layer_visuals: &mut Vec<SQuadLayerVisualsBase> =
                        &mut quad_layer_visuals;
                    let visual_index = quad_layer_visuals.len();
                    quad_layer_visuals.push(SQuadLayerVisualsBase::default());

                    quad_layers.push((visual_index, g, layer_index));
                }
            }
        }

        upload_data
            .tile_layer_uploads
            .resize_with(tile_layer_visuals.len(), || Default::default());

        tile_layer_visuals
            .par_drain(..)
            .zip(upload_data.tile_layer_uploads.par_iter_mut())
            .enumerate()
            .for_each(|(index, (mut visuals, upload_data))| {
                let (visual_index, group_index, layer_index, cur_overlay) = tile_layers[index];

                let group = map.get_group(group_index);
                let layer = map.get_layer(layer_index);

                let mut is_front_layer = false;
                let mut is_switch_layer = false;
                let mut is_tele_layer = false;
                let mut is_speedup_layer = false;
                let mut is_tune_layer = false;
                let mut is_game_layer = false;
                let mut is_entity_layer = false;

                if map.is_game_layer(layer_index) {
                    is_game_layer = true;
                    is_entity_layer = true;
                }

                if map.is_front_layer(layer_index) {
                    is_entity_layer = true;
                    is_front_layer = true;
                }

                if map.is_switch_layer(layer_index) {
                    is_entity_layer = true;
                    is_switch_layer = true;
                }

                if map.is_tele_layer(layer_index) {
                    is_entity_layer = true;
                    is_tele_layer = true;
                }

                if map.is_speedup_layer(layer_index) {
                    is_entity_layer = true;
                    is_speedup_layer = true;
                }

                if map.is_tune_layer(layer_index) {
                    is_entity_layer = true;
                    is_tune_layer = true;
                }

                if let MapLayer::Tile(tile_map) = layer {
                    let mut do_texture_coords = false;
                    if tile_map.0.image == -1 {
                        if is_entity_layer {
                            do_texture_coords = true;
                        }
                    } else {
                        do_texture_coords = true;
                    }

                    if !visuals.init(tile_map.0.width as u32, tile_map.0.height as u32) {
                        return;
                    }

                    visuals.is_textured = do_texture_coords;

                    let mut tmp_tiles: Vec<SGraphicTile> = Vec::new();
                    let mut tmp_tile_tex_coords: Vec<SGraphicTileTexureCoords> = Vec::new();
                    let mut tmp_border_top_tiles: Vec<SGraphicTile> = Vec::new();
                    let mut tmp_border_top_tiles_tex_coords: Vec<SGraphicTileTexureCoords> =
                        Vec::new();
                    let mut tmp_border_left_tiles: Vec<SGraphicTile> = Vec::new();
                    let mut tmp_border_left_tiles_tex_coords: Vec<SGraphicTileTexureCoords> =
                        Vec::new();
                    let mut tmp_border_right_tiles: Vec<SGraphicTile> = Vec::new();
                    let mut tmp_border_right_tiles_tex_coords: Vec<SGraphicTileTexureCoords> =
                        Vec::new();
                    let mut tmp_border_bottom_tiles: Vec<SGraphicTile> = Vec::new();
                    let mut tmp_border_bottom_tiles_tex_coords: Vec<SGraphicTileTexureCoords> =
                        Vec::new();
                    let mut tmp_border_corners: Vec<SGraphicTile> = Vec::new();
                    let mut tmp_border_corners_tex_coords: Vec<SGraphicTileTexureCoords> =
                        Vec::new();

                    if !do_texture_coords {
                        tmp_tiles.reserve(tile_map.0.width as usize * tile_map.0.height as usize);
                        tmp_border_top_tiles.reserve(tile_map.0.width as usize);
                        tmp_border_bottom_tiles.reserve(tile_map.0.width as usize);
                        tmp_border_left_tiles.reserve(tile_map.0.height as usize);
                        tmp_border_right_tiles.reserve(tile_map.0.height as usize);
                        tmp_border_corners.reserve(4);
                    } else {
                        tmp_tile_tex_coords
                            .reserve(tile_map.0.width as usize * tile_map.0.height as usize);
                        tmp_border_top_tiles_tex_coords.reserve(tile_map.0.width as usize);
                        tmp_border_bottom_tiles_tex_coords.reserve(tile_map.0.width as usize);
                        tmp_border_left_tiles_tex_coords.reserve(tile_map.0.height as usize);
                        tmp_border_right_tiles_tex_coords.reserve(tile_map.0.height as usize);
                        tmp_border_corners_tex_coords.reserve(4);
                    }

                    let _x = 0;
                    let _y = 0;
                    let tiles = &tile_map.2;
                    let height = tile_map.0.height as usize;
                    let width = tile_map.0.width as usize;
                    for y in 0..height {
                        for x in 0..width {
                            let mut index: u8 = 0;
                            let mut flags: u8 = 0;
                            let mut angle_rotate = -1;
                            if is_entity_layer {
                                if is_game_layer {
                                    index = tiles[y * width + x].index;
                                    flags = tiles[y * width + x].flags;
                                }
                                if is_front_layer {
                                    index = (tiles)[y * width + x].index;
                                    flags = (tiles)[y * width + x].flags;
                                }
                                if is_switch_layer {
                                    if let MapTileLayerDetail::Switch(switch_tiles) = &tile_map.1 {
                                        flags = 0;
                                        index = switch_tiles[y * width + x].tile_type;
                                        if cur_overlay == 0 {
                                            flags = switch_tiles[y * width + x].flags;
                                            if index == TILE_SWITCHTIMEDOPEN as u8 {
                                                index = 8;
                                            }
                                        } else if cur_overlay == 1 {
                                            index = switch_tiles[y * width + x].number;
                                        } else if cur_overlay == 2 {
                                            index = switch_tiles[y * width + x].delay;
                                        }
                                    }
                                }
                                if is_tele_layer {
                                    if let MapTileLayerDetail::Tele(tele_tiles) = &tile_map.1 {
                                        index = tele_tiles[y * width + x].tile_type;
                                        flags = 0;
                                        if cur_overlay == 1 {
                                            if index != TileNum::TeleCheckIn as u8
                                                && index != TileNum::TeleCheckInEvil as u8
                                            {
                                                index = tele_tiles[y * width + x].number;
                                            } else {
                                                index = 0;
                                            }
                                        }
                                    }
                                }
                                if is_speedup_layer {
                                    if let MapTileLayerDetail::Speedup(speedup_tiles) = &tile_map.1
                                    {
                                        index = speedup_tiles[y * width + x].tile_type;
                                        flags = 0;
                                        angle_rotate = speedup_tiles[y * width + x].angle;
                                        if speedup_tiles[y * width + x].force == 0 {
                                            index = 0;
                                        } else if cur_overlay == 1 {
                                            index = speedup_tiles[y * width + x].force;
                                        } else if cur_overlay == 2 {
                                            index = speedup_tiles[y * width + x].max_speed;
                                        }
                                    }
                                }
                                if is_tune_layer {
                                    if let MapTileLayerDetail::Tune(tune_tiles) = &tile_map.1 {
                                        index = tune_tiles[y * width + x].tile_type;
                                        flags = 0;
                                    }
                                }
                            } else {
                                index = (tiles)[y * width + x].index;
                                flags = (tiles)[y * width + x].flags;
                            }

                            //the amount of tiles handled before this tile
                            let tiles_handled_count = tmp_tiles.len();
                            visuals.tiles_of_layer[y * width + x].set_index_buffer_offset(
                                (tiles_handled_count * 6 * std::mem::size_of::<u32>()) as u32,
                            );

                            let mut add_as_speedup = false;
                            if is_speedup_layer && cur_overlay == 0 {
                                add_as_speedup = true;
                            }

                            if add_tile(
                                &mut tmp_tiles,
                                &mut tmp_tile_tex_coords,
                                index,
                                flags,
                                x as i32,
                                y as i32,
                                group,
                                do_texture_coords,
                                add_as_speedup,
                                angle_rotate,
                            ) {
                                visuals.tiles_of_layer[y * width + x].draw(true);
                            }

                            //do the border tiles
                            if x == 0 {
                                if y == 0 {
                                    visuals.border_top_left.set_index_buffer_offset(
                                        (tmp_border_corners.len() * 6 * std::mem::size_of::<u32>())
                                            as u32,
                                    );
                                    if add_tile(
                                        &mut tmp_border_corners,
                                        &mut tmp_border_corners_tex_coords,
                                        index,
                                        flags,
                                        x as i32,
                                        y as i32,
                                        group,
                                        do_texture_coords,
                                        add_as_speedup,
                                        angle_rotate,
                                    ) {
                                        visuals.border_top_left.draw(true);
                                    }
                                } else if y == height - 1 {
                                    visuals.border_bottom_left.set_index_buffer_offset(
                                        (tmp_border_corners.len() * 6 * std::mem::size_of::<u32>())
                                            as u32,
                                    );
                                    if add_tile(
                                        &mut tmp_border_corners,
                                        &mut tmp_border_corners_tex_coords,
                                        index,
                                        flags,
                                        x as i32,
                                        y as i32,
                                        group,
                                        do_texture_coords,
                                        add_as_speedup,
                                        angle_rotate,
                                    ) {
                                        visuals.border_bottom_left.draw(true);
                                    }
                                } else {
                                    visuals.border_left[y - 1].set_index_buffer_offset(
                                        (tmp_border_left_tiles.len()
                                            * 6
                                            * std::mem::size_of::<u32>())
                                            as u32,
                                    );
                                    if add_tile(
                                        &mut tmp_border_left_tiles,
                                        &mut tmp_border_left_tiles_tex_coords,
                                        index,
                                        flags,
                                        x as i32,
                                        y as i32,
                                        group,
                                        do_texture_coords,
                                        add_as_speedup,
                                        angle_rotate,
                                    ) {
                                        visuals.border_left[y - 1].draw(true);
                                    }
                                }
                            } else if x == width - 1 {
                                if y == 0 {
                                    visuals.border_top_right.set_index_buffer_offset(
                                        (tmp_border_corners.len() * 6 * std::mem::size_of::<u32>())
                                            as u32,
                                    );
                                    if add_tile(
                                        &mut tmp_border_corners,
                                        &mut tmp_border_corners_tex_coords,
                                        index,
                                        flags,
                                        x as i32,
                                        y as i32,
                                        group,
                                        do_texture_coords,
                                        add_as_speedup,
                                        angle_rotate,
                                    ) {
                                        visuals.border_top_right.draw(true);
                                    }
                                } else if y == height - 1 {
                                    visuals.border_bottom_right.set_index_buffer_offset(
                                        (tmp_border_corners.len() * 6 * std::mem::size_of::<u32>())
                                            as u32,
                                    );
                                    if add_tile(
                                        &mut tmp_border_corners,
                                        &mut tmp_border_corners_tex_coords,
                                        index,
                                        flags,
                                        x as i32,
                                        y as i32,
                                        group,
                                        do_texture_coords,
                                        add_as_speedup,
                                        angle_rotate,
                                    ) {
                                        visuals.border_bottom_right.draw(true);
                                    }
                                } else {
                                    visuals.border_right[y - 1].set_index_buffer_offset(
                                        (tmp_border_right_tiles.len()
                                            * 6
                                            * std::mem::size_of::<u32>())
                                            as u32,
                                    );
                                    if add_tile(
                                        &mut tmp_border_right_tiles,
                                        &mut tmp_border_right_tiles_tex_coords,
                                        index,
                                        flags,
                                        x as i32,
                                        y as i32,
                                        group,
                                        do_texture_coords,
                                        add_as_speedup,
                                        angle_rotate,
                                    ) {
                                        visuals.border_right[y - 1].draw(true);
                                    }
                                }
                            } else if y == 0 {
                                if x > 0 && x < width - 1 {
                                    visuals.border_top[x - 1].set_index_buffer_offset(
                                        (tmp_border_top_tiles.len()
                                            * 6
                                            * std::mem::size_of::<u32>())
                                            as u32,
                                    );
                                    if add_tile(
                                        &mut tmp_border_top_tiles,
                                        &mut tmp_border_top_tiles_tex_coords,
                                        index,
                                        flags,
                                        x as i32,
                                        y as i32,
                                        group,
                                        do_texture_coords,
                                        add_as_speedup,
                                        angle_rotate,
                                    ) {
                                        visuals.border_top[x - 1].draw(true);
                                    }
                                }
                            } else if y == height - 1 {
                                if x > 0 && x < width - 1 {
                                    visuals.border_bottom[x - 1].set_index_buffer_offset(
                                        (tmp_border_bottom_tiles.len()
                                            * 6
                                            * std::mem::size_of::<u32>())
                                            as u32,
                                    );
                                    if add_tile(
                                        &mut tmp_border_bottom_tiles,
                                        &mut tmp_border_bottom_tiles_tex_coords,
                                        index,
                                        flags,
                                        x as i32,
                                        y as i32,
                                        group,
                                        do_texture_coords,
                                        add_as_speedup,
                                        angle_rotate,
                                    ) {
                                        visuals.border_bottom[x - 1].draw(true);
                                    }
                                }
                            }
                        }
                    }

                    //append one kill tile to the gamelayer
                    if is_game_layer {
                        visuals.border_kill_tile.set_index_buffer_offset(
                            (tmp_tiles.len() * 6 * std::mem::size_of::<u32>()) as u32,
                        );
                        if add_tile(
                            &mut tmp_tiles,
                            &mut tmp_tile_tex_coords,
                            TileNum::Death as u8,
                            0,
                            0,
                            0,
                            group,
                            do_texture_coords,
                            false,
                            -1,
                        ) {
                            visuals.border_kill_tile.draw(true);
                        }
                    }

                    //add the border corners, then the borders and fix their byte offsets
                    let mut tiles_handled_count = tmp_tiles.len();
                    visuals.border_top_left.add_index_buffer_byte_offset(
                        (tiles_handled_count * 6 * std::mem::size_of::<u32>()) as u32,
                    );
                    visuals.border_top_right.add_index_buffer_byte_offset(
                        (tiles_handled_count * 6 * std::mem::size_of::<u32>()) as u32,
                    );
                    visuals.border_bottom_left.add_index_buffer_byte_offset(
                        (tiles_handled_count * 6 * std::mem::size_of::<u32>()) as u32,
                    );
                    visuals.border_bottom_right.add_index_buffer_byte_offset(
                        (tiles_handled_count * 6 * std::mem::size_of::<u32>()) as u32,
                    );
                    //add the Corners to the tiles
                    tmp_tiles.append(&mut tmp_border_corners);
                    tmp_tile_tex_coords.append(&mut tmp_border_corners_tex_coords);

                    //now the borders
                    tiles_handled_count = tmp_tiles.len();
                    if width > 2 {
                        for i in 0..width - 2 {
                            visuals.border_top[i].add_index_buffer_byte_offset(
                                (tiles_handled_count * 6 * std::mem::size_of::<u32>()) as u32,
                            );
                        }
                    }
                    tmp_tiles.append(&mut tmp_border_top_tiles);
                    tmp_tile_tex_coords.append(&mut tmp_border_top_tiles_tex_coords);

                    tiles_handled_count = tmp_tiles.len();
                    if width > 2 {
                        for i in 0..width - 2 {
                            visuals.border_bottom[i].add_index_buffer_byte_offset(
                                (tiles_handled_count * 6 * std::mem::size_of::<u32>()) as u32,
                            );
                        }
                    }
                    tmp_tiles.append(&mut tmp_border_bottom_tiles);
                    tmp_tile_tex_coords.append(&mut tmp_border_bottom_tiles_tex_coords);

                    tiles_handled_count = tmp_tiles.len();
                    if height > 2 {
                        for i in 0..height - 2 {
                            visuals.border_left[i].add_index_buffer_byte_offset(
                                (tiles_handled_count * 6 * std::mem::size_of::<u32>()) as u32,
                            );
                        }
                    }
                    tmp_tiles.append(&mut tmp_border_left_tiles);
                    tmp_tile_tex_coords.append(&mut tmp_border_left_tiles_tex_coords);

                    tiles_handled_count = tmp_tiles.len();
                    if height > 2 {
                        for i in 0..height - 2 {
                            visuals.border_right[i].add_index_buffer_byte_offset(
                                (tiles_handled_count * 6 * std::mem::size_of::<u32>()) as u32,
                            );
                        }
                    }
                    tmp_tiles.append(&mut tmp_border_right_tiles);
                    tmp_tile_tex_coords.append(&mut tmp_border_right_tiles_tex_coords);

                    let upload_data_size = tmp_tile_tex_coords.len()
                        * std::mem::size_of::<SGraphicTileTexureCoords>()
                        + tmp_tiles.len() * std::mem::size_of::<SGraphicTile>();
                    if upload_data_size > 0 {
                        let mut upload_data_buffer = graphics_mt
                            .mem_alloc(GraphicsMemoryAllocationType::Buffer, upload_data_size);

                        mem_copy_special(
                            upload_data_buffer.as_mut_slice(),
                            &tmp_tiles,
                            std::mem::size_of::<vec2>(),
                            tmp_tiles.len() * 4,
                            if do_texture_coords {
                                std::mem::size_of::<vec3>()
                            } else {
                                0
                            },
                        );
                        if do_texture_coords {
                            mem_copy_special(
                                upload_data_buffer
                                    .as_mut_slice()
                                    .split_at_mut(std::mem::size_of::<vec2>())
                                    .1,
                                &tmp_tile_tex_coords,
                                std::mem::size_of::<vec3>(),
                                tmp_tiles.len() * 4,
                                std::mem::size_of::<vec2>(),
                            );
                        }

                        *upload_data = (
                            visual_index,
                            upload_data_buffer,
                            do_texture_coords,
                            tmp_tiles.len() * 6,
                            visuals,
                        );
                    }
                }
            });

        upload_data
            .quad_layer_uploads
            .resize_with(quad_layer_visuals.len(), || Default::default());

        quad_layer_visuals
            .par_drain(..)
            .zip(upload_data.quad_layer_uploads.par_iter_mut())
            .enumerate()
            .for_each(|(index, (q_layer_visuals, upload_data))| {
                let (visual_index, group_index, layer_index) = quad_layers[index];
                let _group = map.get_group(group_index);
                let layer = map.get_layer(layer_index);

                if let MapLayer::Quads(q_layer) = layer {
                    let is_textured = q_layer.0.image != -1;

                    let mut tmp_quads: Vec<STmpQuad> = Vec::new();
                    let mut tmp_quads_textured: Vec<STmpQuadTextured> = Vec::new();

                    if is_textured {
                        tmp_quads_textured.resize(q_layer.0.num_quads as usize, Default::default());
                    } else {
                        tmp_quads.resize(q_layer.0.num_quads as usize, Default::default());
                    }

                    let quads = &q_layer.1;
                    quads.iter().enumerate().for_each(|(i, quad)| {
                        for j in 0..4 {
                            let mut quad_index = j;
                            if j == 2 {
                                quad_index = 3;
                            } else if j == 3 {
                                quad_index = 2;
                            }
                            if !is_textured {
                                // ignore the conversion for the position coordinates
                                tmp_quads[i].vertices[j].x = quad.points[quad_index].x as f32;
                                tmp_quads[i].vertices[j].y = quad.points[quad_index].y as f32;
                                tmp_quads[i].vertices[j].center_x = quad.points[4].x as f32;
                                tmp_quads[i].vertices[j].center_y = quad.points[4].y as f32;
                                tmp_quads[i].vertices[j].r = quad.colors[quad_index].r() as u8;
                                tmp_quads[i].vertices[j].g = quad.colors[quad_index].g() as u8;
                                tmp_quads[i].vertices[j].b = quad.colors[quad_index].b() as u8;
                                tmp_quads[i].vertices[j].a = quad.colors[quad_index].a() as u8;
                            } else {
                                // ignore the conversion for the position coordinates
                                tmp_quads_textured[i].vertices[j].x =
                                    quad.points[quad_index].x as f32;
                                tmp_quads_textured[i].vertices[j].y =
                                    quad.points[quad_index].y as f32;
                                tmp_quads_textured[i].vertices[j].center_x =
                                    quad.points[4].x as f32;
                                tmp_quads_textured[i].vertices[j].center_y =
                                    quad.points[4].y as f32;
                                tmp_quads_textured[i].vertices[j].u =
                                    fx2f(quad.tex_coords[quad_index].x);
                                tmp_quads_textured[i].vertices[j].v =
                                    fx2f(quad.tex_coords[quad_index].y);
                                tmp_quads_textured[i].vertices[j].r =
                                    quad.colors[quad_index].r() as u8;
                                tmp_quads_textured[i].vertices[j].g =
                                    quad.colors[quad_index].g() as u8;
                                tmp_quads_textured[i].vertices[j].b =
                                    quad.colors[quad_index].b() as u8;
                                tmp_quads_textured[i].vertices[j].a =
                                    quad.colors[quad_index].a() as u8;
                            }
                        }
                    });

                    let upload_data_size;
                    if is_textured {
                        upload_data_size =
                            tmp_quads_textured.len() * std::mem::size_of::<STmpQuadTextured>();
                    } else {
                        upload_data_size = tmp_quads.len() * std::mem::size_of::<STmpQuad>();
                    }

                    if upload_data_size > 0 {
                        let mut upload_data_buffer = graphics_mt.mem_alloc(
                            GraphicsMemoryAllocationType::Buffer,
                            if is_textured {
                                tmp_quads_textured.len() * std::mem::size_of::<STmpQuadTextured>()
                            } else {
                                tmp_quads.len() * std::mem::size_of::<STmpQuad>()
                            },
                        );

                        if is_textured {
                            tmp_quads_textured
                                .iter()
                                .enumerate()
                                .for_each(|(index, q)| {
                                    q.copy_into_slice(
                                        upload_data_buffer
                                            .as_mut_slice()
                                            .split_at_mut(
                                                index * std::mem::size_of::<STmpQuadTextured>(),
                                            )
                                            .1,
                                        0,
                                    )
                                });

                            upload_data_buffer
                                .as_mut_slice()
                                .par_chunks_exact_mut(std::mem::size_of::<STmpQuadTextured>())
                                .enumerate()
                                .for_each(|(i, q_slice)| {
                                    tmp_quads_textured[i].copy_into_slice(q_slice, 0)
                                });
                        } else {
                            upload_data_buffer
                                .as_mut_slice()
                                .par_chunks_exact_mut(std::mem::size_of::<STmpQuad>())
                                .enumerate()
                                .for_each(|(i, q_slice)| tmp_quads[i].copy_into_slice(q_slice, 0));
                        }

                        *upload_data = (
                            visual_index,
                            upload_data_buffer,
                            is_textured,
                            (q_layer.0.num_quads * 6) as usize,
                            q_layer_visuals,
                        );
                    }
                }
            });

        return upload_data;
    }

    pub fn upload_map(
        &mut self,
        graphics: &mut Graphics,
        mut upload_data: ClientMapBufferUploadData,
    ) {
        upload_data.tile_layer_uploads.drain(..).for_each(
            |(visual_index, raw_data, textured, indices_count, visuals)| {
                if raw_data.is_error() || raw_data.as_slice().is_empty() {
                    self.tile_layer_visuals.push(STileLayerVisuals {
                        base: visuals,
                        buffer_container_index: None,
                    });
                } else {
                    // first create the buffer object
                    let buffer_object_index =
                        graphics.create_buffer_object(raw_data.as_slice().len(), raw_data, 0);

                    // then create the buffer container
                    let mut container_info = SBufferContainerInfo {
                        stride: if textured {
                            std::mem::size_of::<f32>() * 2 + std::mem::size_of::<vec3>()
                        } else {
                            0
                        },
                        vert_buffer_binding_index: buffer_object_index.clone(),
                        attributes: Vec::new(),
                    };
                    container_info.attributes.push(SAttribute::default());
                    let mut cont_attr = container_info.attributes.last_mut().unwrap();
                    cont_attr.data_type_count = 2;
                    cont_attr.graphics_type = GraphicsType::Float;
                    cont_attr.normalized = false;
                    cont_attr.offset = 0;
                    cont_attr.func_type = 0;
                    if textured {
                        container_info.attributes.push(SAttribute::default());
                        cont_attr = container_info.attributes.last_mut().unwrap();
                        cont_attr.data_type_count = 3;
                        cont_attr.graphics_type = GraphicsType::Float;
                        cont_attr.normalized = false;
                        cont_attr.offset = std::mem::size_of::<vec2>();
                        cont_attr.func_type = 0;
                    }

                    self.tile_layer_visuals.push(STileLayerVisuals {
                        base: visuals,
                        buffer_container_index: Some(
                            graphics.create_buffer_container(&container_info),
                        ),
                    });
                    // and finally inform the backend how many indices are required
                    graphics.indices_num_required_notify(indices_count);
                }
            },
        );

        upload_data.quad_layer_uploads.drain(..).for_each(
            |(visual_index, raw_data, textured, indices_count, visuals)| {
                if raw_data.is_error() || raw_data.as_slice().is_empty() {
                    self.quad_layer_visuals.push(SQuadLayerVisuals {
                        base: visuals,
                        buffer_container_index: None,
                    });
                } else {
                    // create the buffer object
                    let buffer_object_index =
                        graphics.create_buffer_object(raw_data.as_slice().len(), raw_data, 0);
                    // then create the buffer container
                    let mut container_info = SBufferContainerInfo {
                        stride: if textured {
                            std::mem::size_of::<STmpQuadTextured>() / 4
                        } else {
                            std::mem::size_of::<STmpQuad>() / 4
                        },
                        vert_buffer_binding_index: buffer_object_index.clone(),
                        attributes: Vec::new(),
                    };
                    container_info.attributes.push(SAttribute::default());
                    let mut cont_attr = container_info.attributes.last_mut().unwrap();
                    cont_attr.data_type_count = 4;
                    cont_attr.graphics_type = GraphicsType::Float;
                    cont_attr.normalized = false;
                    cont_attr.offset = 0;
                    cont_attr.func_type = 0;
                    container_info.attributes.push(SAttribute::default());
                    cont_attr = container_info.attributes.last_mut().unwrap();
                    cont_attr.data_type_count = 4;
                    cont_attr.graphics_type = GraphicsType::UnsignedByte;
                    cont_attr.normalized = true;
                    cont_attr.offset = std::mem::size_of::<f32>() * 4;
                    cont_attr.func_type = 0;
                    if textured {
                        container_info.attributes.push(SAttribute::default());
                        cont_attr = container_info.attributes.last_mut().unwrap();
                        cont_attr.data_type_count = 2;
                        cont_attr.graphics_type = GraphicsType::Float;
                        cont_attr.normalized = false;
                        cont_attr.offset =
                            std::mem::size_of::<f32>() * 4 + std::mem::size_of::<u8>() * 4;
                        cont_attr.func_type = 0;
                    }

                    self.quad_layer_visuals.push(SQuadLayerVisuals {
                        base: visuals,
                        buffer_container_index: Some(
                            graphics.create_buffer_container(&container_info),
                        ),
                    });
                    // and finally inform the backend how many indices are required
                    graphics.indices_num_required_notify(indices_count);
                }
            },
        )
    }
}
