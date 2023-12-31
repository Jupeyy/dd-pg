use std::ops::{Index, IndexMut};

use graphics::{
    buffer_object_handle::BufferObjectIndex, graphics::Graphics, graphics_mt::GraphicsMultiThreaded,
};
use rayon::{
    prelude::{
        IndexedParallelIterator, IntoParallelRefMutIterator, ParallelDrainRange, ParallelIterator,
    },
    slice::ParallelSliceMut,
};

use shared_base::{
    datafile::CDatafileWrapper,
    mapdef::{
        rotation_180, rotation_270, MapLayer, MapTileLayerDetail, TileFlag, TileNum, ROTATION_90,
        TILE_SWITCHTIMEDOPEN,
    },
};

use math::math::{
    fx2f,
    vector::{ivec2, ubvec4, vec2},
};

use graphics_types::types::{GraphicsBackendMemory, GraphicsMemoryAllocationType};

#[derive(Copy, Clone, Default)]
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

#[derive(Default)]
pub struct TileLayerVisualsBase {
    pub tiles_of_layer: Vec<TileVisual>,

    pub border_top_left: TileVisual,
    pub border_top_right: TileVisual,
    pub border_bottom_right: TileVisual,
    pub border_bottom_left: TileVisual,

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

pub struct TileLayerVisuals {
    pub base: TileLayerVisualsBase,
    pub buffer_object_index: Option<BufferObjectIndex>,
}

#[derive(Copy, Clone, Default)]
pub struct QuadVisual {
    pub index_buffer_byte_offset: usize,
}

#[derive(Default)]
pub struct QuadLayerVisualsBase {
    pub quad_num: usize,
    pub quads_of_layer: Vec<QuadVisual>,

    pub is_textured: bool,
}

impl QuadLayerVisualsBase {
    pub fn new() -> Self {
        Default::default()
    }
}

pub struct QuadLayerVisuals {
    pub base: QuadLayerVisualsBase,

    pub buffer_object_index: Option<BufferObjectIndex>,
}

trait UploadDataAsBytes {
    fn copy_into_slice(&self, dest: &mut [u8], index: usize);
}

#[repr(C)]
#[derive(Default)]
struct GraphicTile {
    top_left: vec2,
    top_right: vec2,
    bottom_right: vec2,
    bottom_left: vec2,
}

impl Index<usize> for GraphicTile {
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

impl UploadDataAsBytes for GraphicTile {
    fn copy_into_slice(&self, dest: &mut [u8], index: usize) {
        let mut off: usize = 0;
        let coord: &vec2 = match index {
            0 => &self.top_left,
            1 => &self.top_right,
            2 => &self.bottom_right,
            3 => &self.bottom_left,
            _ => panic!("out of bounds"),
        };

        coord.x.to_ne_bytes().iter().for_each(|byte| {
            dest[off] = *byte;
            off += 1;
        });
        coord.y.to_ne_bytes().iter().for_each(|byte| {
            dest[off] = *byte;
            off += 1;
        });
    }
}

impl IndexMut<usize> for GraphicTile {
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
struct GraphicTileTexureCoords {
    tex_coord_top_left: ubvec4,
    tex_coord_top_right: ubvec4,
    tex_coord_bottom_right: ubvec4,
    tex_coord_bottom_left: ubvec4,
}

impl UploadDataAsBytes for GraphicTileTexureCoords {
    fn copy_into_slice(&self, dest: &mut [u8], index: usize) {
        let mut off: usize = 0;
        let tex_coord: &ubvec4 = match index {
            0 => &self.tex_coord_top_left,
            1 => &self.tex_coord_top_right,
            2 => &self.tex_coord_bottom_right,
            3 => &self.tex_coord_bottom_left,
            _ => panic!("out of bounds"),
        };

        dest[off] = tex_coord.x;
        off += 1;
        dest[off] = tex_coord.y;
        off += 1;
        dest[off] = tex_coord.z;
        off += 1;
        dest[off] = tex_coord.w;
    }
}

fn fill_tmp_tile_speedup(
    tmp_tile: &mut GraphicTile,
    tmp_tex: Option<&mut GraphicTileTexureCoords>,
    _flags: u8,
    _index: u8,
    x: i32,
    y: i32,
    offset: &ivec2,
    scale: i32,
    angle_rotate: i16,
) {
    let angle = angle_rotate % 360;
    fill_tmp_tile(
        tmp_tile,
        tmp_tex,
        if angle >= 270 {
            rotation_270().bits() as u8
        } else {
            if angle >= 180 {
                rotation_180().bits() as u8
            } else {
                if angle >= 90 {
                    ROTATION_90.bits() as u8
                } else {
                    0
                }
            }
        },
        (angle_rotate % 90) as u8,
        x,
        y,
        offset,
        scale,
    );
}

fn fill_tmp_tile(
    tmp_tile: &mut GraphicTile,
    tmp_tex: Option<&mut GraphicTileTexureCoords>,
    flags: u8,
    index: u8,
    x: i32,
    y: i32,
    offset: &ivec2,
    scale: i32,
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

        if (flags & TileFlag::XFLIP.bits() as u8) != 0 {
            x0 = x2;
            x1 = x3;
            x2 = x3;
            x3 = x0;
        }

        if (flags & TileFlag::YFLIP.bits() as u8) != 0 {
            y0 = y3;
            y2 = y1;
            y3 = y1;
            y1 = y0;
        }

        if (flags & TileFlag::ROTATE.bits() as u8) != 0 {
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

        tmp_tex.tex_coord_top_left.x = x0;
        tmp_tex.tex_coord_top_left.y = y0;
        tmp_tex.tex_coord_bottom_left.x = x3;
        tmp_tex.tex_coord_bottom_left.y = y3;
        tmp_tex.tex_coord_top_right.x = x1;
        tmp_tex.tex_coord_top_right.y = y1;
        tmp_tex.tex_coord_bottom_right.x = x2;
        tmp_tex.tex_coord_bottom_right.y = y2;

        tmp_tex.tex_coord_top_left.z = index;
        tmp_tex.tex_coord_bottom_left.z = index;
        tmp_tex.tex_coord_top_right.z = index;
        tmp_tex.tex_coord_bottom_right.z = index;

        let has_rotation = (flags & TileFlag::ROTATE.bits() as u8) != 0;
        tmp_tex.tex_coord_top_left.w = has_rotation as u8;
        tmp_tex.tex_coord_bottom_left.w = has_rotation as u8;
        tmp_tex.tex_coord_top_right.w = has_rotation as u8;
        tmp_tex.tex_coord_bottom_right.w = has_rotation as u8;
    }

    tmp_tile.top_left.x = (x * scale) as f32 + offset.x as f32;
    tmp_tile.top_left.y = (y * scale) as f32 + offset.y as f32;
    tmp_tile.bottom_left.x = (x * scale) as f32 + offset.x as f32;
    tmp_tile.bottom_left.y = (y * scale + scale) as f32 + offset.y as f32;
    tmp_tile.top_right.x = (x * scale + scale) as f32 + offset.x as f32;
    tmp_tile.top_right.y = (y * scale) as f32 + offset.y as f32;
    tmp_tile.bottom_right.x = (x * scale + scale) as f32 + offset.x as f32;
    tmp_tile.bottom_right.y = (y * scale + scale) as f32 + offset.y as f32;
}

fn add_tile(
    tmp_tiles: &mut Vec<GraphicTile>,
    tmp_tile_tex_coords: &mut Vec<GraphicTileTexureCoords>,
    index: u8,
    flags: u8,
    x: i32,
    y: i32,
    do_texture_coords: bool,
    fill_speedup: bool,
    angle_rotate: i16,
    offset: &ivec2,
) -> bool {
    if index > 0 {
        tmp_tiles.push(GraphicTile::default());
        let tile = tmp_tiles.last_mut().unwrap();
        let mut tile_tex: Option<&mut GraphicTileTexureCoords> = None;
        if do_texture_coords {
            tmp_tile_tex_coords.push(GraphicTileTexureCoords::default());
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
                offset,
                32,
                angle_rotate as i16,
            );
        } else {
            fill_tmp_tile(tile, tile_tex, flags, index, x, y, offset, 32);
        }

        return true;
    }
    return false;
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

impl UploadDataAsBytes for TmpQuadVertexTextured {
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
struct TmpQuadVertex {
    x: f32,
    y: f32,
    center_x: f32,
    center_y: f32,
    r: u8,
    g: u8,
    b: u8,
    a: u8,
}

impl UploadDataAsBytes for TmpQuadVertex {
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
struct TmpQuad {
    vertices: [TmpQuadVertex; 4],
}

impl UploadDataAsBytes for TmpQuad {
    fn copy_into_slice(&self, dest: &mut [u8], index: usize /* ignore */) {
        let mut off: usize = 0;
        self.vertices.iter().for_each(|v| {
            v.copy_into_slice(dest.split_at_mut(off).1, index);
            off += std::mem::size_of::<TmpQuadVertex>()
        });
    }
}

#[repr(C)]
#[derive(Clone, Default)]
struct TmpQuadTextured {
    vertices: [TmpQuadVertexTextured; 4],
}

impl UploadDataAsBytes for TmpQuadTextured {
    fn copy_into_slice(&self, dest: &mut [u8], index: usize /* ignore */) {
        let mut off: usize = 0;
        self.vertices.iter().for_each(|v| {
            v.copy_into_slice(dest.split_at_mut(off).1, index);
            off += std::mem::size_of::<TmpQuadVertexTextured>()
        });
    }
}

fn mem_copy_special<T: UploadDataAsBytes + Send + Sync>(
    dest: &mut [u8],
    src: &[T],
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

#[derive(Debug, Copy, Clone)]
pub enum MapRenderTextOverlayType {
    Top,
    Bottom,
    Center,
}

#[derive(Debug, Default, Copy, Clone)]
pub struct MapRenderInfo {
    pub visual_index: usize,
    pub group_index: usize,
    pub layer_index: usize,
    pub cur_text_overlay: Option<MapRenderTextOverlayType>,
    pub is_physics_layer: bool,
}

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
pub struct ClientMapBufferedVisuals {
    pub tile_layer_visuals: Vec<TileLayerVisuals>,
    pub quad_layer_visuals: Vec<QuadLayerVisuals>,
}

#[derive(Default)]
pub struct ClientMapBufferedInfo {
    pub main_physics_layer_group_index: usize,
    pub main_physics_layer_layer_index: usize,
}

#[derive(Default)]
pub struct ClientMapBufferedRenderProcess {
    pub background_render_layers: Vec<MapRenderLayer>,
    pub foreground_render_layers: Vec<MapRenderLayer>,
}

#[derive(Default)]
pub struct ClientMapBuffered {
    pub visuals: ClientMapBufferedVisuals,
    pub info: ClientMapBufferedInfo,
    pub render: ClientMapBufferedRenderProcess,
}

#[derive(Default)]
pub struct ClientMapBufferUploadData {
    pub tile_layer_uploads: Vec<(
        usize,
        Option<GraphicsBackendMemory>,
        bool,
        u64,
        TileLayerVisualsBase,
        MapRenderInfo,
    )>,
    pub quad_layer_uploads: Vec<(
        usize,
        Option<GraphicsBackendMemory>,
        bool,
        u64,
        QuadLayerVisualsBase,
        MapRenderInfo,
    )>,
    pub main_physics_layer_group_index: usize,
    pub main_physics_layer_layer_index: usize,
}

impl ClientMapBuffered {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn prepare_upload(
        graphics_mt: &GraphicsMultiThreaded,
        map: &CDatafileWrapper,
        background_only: bool,
    ) -> ClientMapBufferUploadData {
        let mut upload_data = ClientMapBufferUploadData::default();

        let mut tile_layer_visuals: Vec<TileLayerVisualsBase> = Vec::new();
        let mut quad_layer_visuals: Vec<QuadLayerVisualsBase> = Vec::new();

        let mut passed_main_physics_layer = false;

        //prepare all visuals for all tile layers
        let mut tile_layers: Vec<(usize, usize, usize, usize)> = Vec::new();
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

                if map.is_game_layer(layer_index) {
                    passed_main_physics_layer = true;

                    upload_data.main_physics_layer_group_index = g;
                    upload_data.main_physics_layer_layer_index = l;
                }

                if map.is_switch_layer(layer_index) {
                    is_switch_layer = true;
                }

                if map.is_tele_layer(layer_index) {
                    is_tele_layer = true;
                }

                if map.is_speedup_layer(layer_index) {
                    is_speedup_layer = true;
                }

                if background_only {
                    if passed_main_physics_layer {
                        return upload_data;
                    }
                }

                if let MapLayer::Tile(_) = layer {
                    let mut text_overlay_count = 0;
                    if is_switch_layer {
                        text_overlay_count = 2;
                    } else if is_tele_layer {
                        text_overlay_count = 1;
                    } else if is_speedup_layer {
                        text_overlay_count = 2;
                    }

                    let mut cur_text_overlay = 0;
                    while cur_text_overlay < text_overlay_count + 1 {
                        // We can later just count the tile layers to get the idx in the vector
                        let tile_layer_visuals = &mut tile_layer_visuals;
                        let visual_index = tile_layer_visuals.len();
                        tile_layer_visuals.push(TileLayerVisualsBase::default());

                        tile_layers.push((visual_index, g, layer_index, cur_text_overlay));

                        cur_text_overlay += 1;
                    }
                } else if let MapLayer::Quads(_q_layer) = layer {
                    let quad_layer_visuals: &mut Vec<QuadLayerVisualsBase> =
                        &mut quad_layer_visuals;
                    let visual_index = quad_layer_visuals.len();
                    quad_layer_visuals.push(QuadLayerVisualsBase::default());

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
                let (visual_index, group_index, layer_index, cur_text_overlay) = tile_layers[index];

                let group = map.get_group(group_index);
                let layer = map.get_layer(layer_index);

                let mut is_front_layer = false;
                let mut is_switch_layer = false;
                let mut is_tele_layer = false;
                let mut is_speedup_layer = false;
                let mut is_tune_layer = false;
                let mut is_game_layer = false;
                let mut is_physics_layer = false;

                let mut text_overlay_type = None;

                if map.is_game_layer(layer_index) {
                    is_game_layer = true;
                    is_physics_layer = true;
                }

                if map.is_front_layer(layer_index) {
                    is_physics_layer = true;
                    is_front_layer = true;
                }

                if map.is_switch_layer(layer_index) {
                    is_physics_layer = true;
                    is_switch_layer = true;
                }

                if map.is_tele_layer(layer_index) {
                    is_physics_layer = true;
                    is_tele_layer = true;
                }

                if map.is_speedup_layer(layer_index) {
                    is_physics_layer = true;
                    is_speedup_layer = true;
                }

                if map.is_tune_layer(layer_index) {
                    is_physics_layer = true;
                    is_tune_layer = true;
                }

                if let MapLayer::Tile(tile_map) = layer {
                    let mut do_texture_coords = false;
                    if tile_map.0.image == -1 {
                        if is_physics_layer {
                            do_texture_coords = true;
                        }
                    } else {
                        do_texture_coords = true;
                    }

                    if !visuals.init(tile_map.0.width as u32, tile_map.0.height as u32) {
                        return;
                    }

                    visuals.is_textured = do_texture_coords;

                    let mut tmp_tiles: Vec<GraphicTile> = Vec::new();
                    let mut tmp_tile_tex_coords: Vec<GraphicTileTexureCoords> = Vec::new();
                    let mut tmp_border_top_tiles: Vec<GraphicTile> = Vec::new();
                    let mut tmp_border_top_tiles_tex_coords: Vec<GraphicTileTexureCoords> =
                        Vec::new();
                    let mut tmp_border_left_tiles: Vec<GraphicTile> = Vec::new();
                    let mut tmp_border_left_tiles_tex_coords: Vec<GraphicTileTexureCoords> =
                        Vec::new();
                    let mut tmp_border_right_tiles: Vec<GraphicTile> = Vec::new();
                    let mut tmp_border_right_tiles_tex_coords: Vec<GraphicTileTexureCoords> =
                        Vec::new();
                    let mut tmp_border_bottom_tiles: Vec<GraphicTile> = Vec::new();
                    let mut tmp_border_bottom_tiles_tex_coords: Vec<GraphicTileTexureCoords> =
                        Vec::new();
                    let mut tmp_border_corners: Vec<GraphicTile> = Vec::new();
                    let mut tmp_border_corners_tex_coords: Vec<GraphicTileTexureCoords> =
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
                            if is_physics_layer {
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
                                        if cur_text_overlay == 0 {
                                            flags = switch_tiles[y * width + x].flags;
                                            if index == TILE_SWITCHTIMEDOPEN as u8 {
                                                index = 8;
                                            }
                                        } else if cur_text_overlay == 1 {
                                            index = switch_tiles[y * width + x].number;
                                            text_overlay_type =
                                                Some(MapRenderTextOverlayType::Bottom);
                                        } else if cur_text_overlay == 2 {
                                            index = switch_tiles[y * width + x].delay;
                                            text_overlay_type = Some(MapRenderTextOverlayType::Top);
                                        }
                                    }
                                }
                                if is_tele_layer {
                                    if let MapTileLayerDetail::Tele(tele_tiles) = &tile_map.1 {
                                        index = tele_tiles[y * width + x].tile_type;
                                        flags = 0;
                                        if cur_text_overlay == 1 {
                                            if index != TileNum::TeleCheckIn as u8
                                                && index != TileNum::TeleCheckInEvil as u8
                                            {
                                                index = tele_tiles[y * width + x].number;
                                            } else {
                                                index = 0;
                                            }
                                            text_overlay_type =
                                                Some(MapRenderTextOverlayType::Center);
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
                                        } else if cur_text_overlay == 1 {
                                            index = speedup_tiles[y * width + x].force;
                                            text_overlay_type =
                                                Some(MapRenderTextOverlayType::Bottom);
                                        } else if cur_text_overlay == 2 {
                                            index = speedup_tiles[y * width + x].max_speed;
                                            text_overlay_type = Some(MapRenderTextOverlayType::Top);
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
                            visuals.tiles_of_layer[y * width + x]
                                .set_index_buffer_offset_quad(tiles_handled_count as u32);

                            let mut add_as_speedup = false;
                            if is_speedup_layer && cur_text_overlay == 0 {
                                add_as_speedup = true;
                            }

                            if add_tile(
                                &mut tmp_tiles,
                                &mut tmp_tile_tex_coords,
                                index,
                                flags,
                                x as i32,
                                y as i32,
                                do_texture_coords,
                                add_as_speedup,
                                angle_rotate,
                                &ivec2::default(),
                            ) {
                                visuals.tiles_of_layer[y * width + x].set_drawable(true);
                            }

                            //do the border tiles
                            if x == 0 {
                                if y == 0 {
                                    visuals.border_top_left.set_index_buffer_offset_quad(
                                        tmp_border_corners.len() as u32,
                                    );
                                    if add_tile(
                                        &mut tmp_border_corners,
                                        &mut tmp_border_corners_tex_coords,
                                        index,
                                        flags,
                                        0,
                                        0,
                                        do_texture_coords,
                                        add_as_speedup,
                                        angle_rotate,
                                        &ivec2::new(-32, -32),
                                    ) {
                                        visuals.border_top_left.set_drawable(true);
                                    }
                                } else if y == height - 1 {
                                    visuals.border_bottom_left.set_index_buffer_offset_quad(
                                        tmp_border_corners.len() as u32,
                                    );
                                    if add_tile(
                                        &mut tmp_border_corners,
                                        &mut tmp_border_corners_tex_coords,
                                        index,
                                        flags,
                                        0,
                                        0,
                                        do_texture_coords,
                                        add_as_speedup,
                                        angle_rotate,
                                        &ivec2::new(-32, 0),
                                    ) {
                                        visuals.border_bottom_left.set_drawable(true);
                                    }
                                }
                                visuals.border_left[y].set_index_buffer_offset_quad(
                                    tmp_border_left_tiles.len() as u32,
                                );
                                if add_tile(
                                    &mut tmp_border_left_tiles,
                                    &mut tmp_border_left_tiles_tex_coords,
                                    index,
                                    flags,
                                    0,
                                    y as i32,
                                    do_texture_coords,
                                    add_as_speedup,
                                    angle_rotate,
                                    &ivec2::new(-32, 0),
                                ) {
                                    visuals.border_left[y].set_drawable(true);
                                }
                            } else if x == width - 1 {
                                if y == 0 {
                                    visuals.border_top_right.set_index_buffer_offset_quad(
                                        tmp_border_corners.len() as u32,
                                    );
                                    if add_tile(
                                        &mut tmp_border_corners,
                                        &mut tmp_border_corners_tex_coords,
                                        index,
                                        flags,
                                        0,
                                        0,
                                        do_texture_coords,
                                        add_as_speedup,
                                        angle_rotate,
                                        &ivec2::new(0, -32),
                                    ) {
                                        visuals.border_top_right.set_drawable(true);
                                    }
                                } else if y == height - 1 {
                                    visuals.border_bottom_right.set_index_buffer_offset_quad(
                                        tmp_border_corners.len() as u32,
                                    );
                                    if add_tile(
                                        &mut tmp_border_corners,
                                        &mut tmp_border_corners_tex_coords,
                                        index,
                                        flags,
                                        0,
                                        0,
                                        do_texture_coords,
                                        add_as_speedup,
                                        angle_rotate,
                                        &ivec2::new(0, 0),
                                    ) {
                                        visuals.border_bottom_right.set_drawable(true);
                                    }
                                }
                                visuals.border_right[y].set_index_buffer_offset_quad(
                                    tmp_border_right_tiles.len() as u32,
                                );
                                if add_tile(
                                    &mut tmp_border_right_tiles,
                                    &mut tmp_border_right_tiles_tex_coords,
                                    index,
                                    flags,
                                    0,
                                    y as i32,
                                    do_texture_coords,
                                    add_as_speedup,
                                    angle_rotate,
                                    &ivec2::new(0, 0),
                                ) {
                                    visuals.border_right[y].set_drawable(true);
                                }
                            }
                            if y == 0 {
                                visuals.border_top[x].set_index_buffer_offset_quad(
                                        tmp_border_top_tiles.len() as u32,
                                    );
                                if add_tile(
                                    &mut tmp_border_top_tiles,
                                    &mut tmp_border_top_tiles_tex_coords,
                                    index,
                                    flags,
                                    x as i32,
                                    0,
                                    do_texture_coords,
                                    add_as_speedup,
                                    angle_rotate,
                                    &ivec2::new(0, -32),
                                ) {
                                    visuals.border_top[x].set_drawable(true);
                                }
                            } else if y == height - 1 {
                                visuals.border_bottom[x].set_index_buffer_offset_quad(
                                    tmp_border_bottom_tiles.len() as u32,
                                );
                                if add_tile(
                                    &mut tmp_border_bottom_tiles,
                                    &mut tmp_border_bottom_tiles_tex_coords,
                                    index,
                                    flags,
                                    x as i32,
                                    0,
                                    do_texture_coords,
                                    add_as_speedup,
                                    angle_rotate,
                                    &ivec2::new(0, 0),
                                ) {
                                    visuals.border_bottom[x].set_drawable(true);
                                }
                            }
                        }
                    }

                    //append one kill tile to the gamelayer
                    if is_game_layer {
                        visuals
                            .border_kill_tile
                            .set_index_buffer_offset_quad(tmp_tiles.len() as u32);
                        if add_tile(
                            &mut tmp_tiles,
                            &mut tmp_tile_tex_coords,
                            TileNum::Death as u8,
                            0,
                            0,
                            0,
                            do_texture_coords,
                            false,
                            -1,
                            &ivec2::new(0, 0),
                        ) {
                            visuals.border_kill_tile.set_drawable(true);
                        }
                    }

                    //add the border corners, then the borders and fix their byte offsets
                    let mut tiles_handled_count = tmp_tiles.len();
                    visuals
                        .border_top_left
                        .add_index_buffer_offset_quad(tiles_handled_count as u32);
                    visuals
                        .border_top_right
                        .add_index_buffer_offset_quad(tiles_handled_count as u32);
                    visuals
                        .border_bottom_left
                        .add_index_buffer_offset_quad(tiles_handled_count as u32);
                    visuals
                        .border_bottom_right
                        .add_index_buffer_offset_quad(tiles_handled_count as u32);
                    //add the Corners to the tiles
                    tmp_tiles.append(&mut tmp_border_corners);
                    tmp_tile_tex_coords.append(&mut tmp_border_corners_tex_coords);

                    //now the borders
                    tiles_handled_count = tmp_tiles.len();
                    for i in 0..width {
                        visuals.border_top[i]
                            .add_index_buffer_offset_quad(tiles_handled_count as u32);
                    }
                    tmp_tiles.append(&mut tmp_border_top_tiles);
                    tmp_tile_tex_coords.append(&mut tmp_border_top_tiles_tex_coords);

                    tiles_handled_count = tmp_tiles.len();
                    for i in 0..width {
                        visuals.border_bottom[i]
                            .add_index_buffer_offset_quad(tiles_handled_count as u32);
                    }
                    tmp_tiles.append(&mut tmp_border_bottom_tiles);
                    tmp_tile_tex_coords.append(&mut tmp_border_bottom_tiles_tex_coords);

                    tiles_handled_count = tmp_tiles.len();
                    for i in 0..height {
                        visuals.border_left[i]
                            .add_index_buffer_offset_quad(tiles_handled_count as u32);
                    }
                    tmp_tiles.append(&mut tmp_border_left_tiles);
                    tmp_tile_tex_coords.append(&mut tmp_border_left_tiles_tex_coords);

                    tiles_handled_count = tmp_tiles.len();
                    for i in 0..height {
                        visuals.border_right[i]
                            .add_index_buffer_offset_quad(tiles_handled_count as u32);
                    }
                    tmp_tiles.append(&mut tmp_border_right_tiles);
                    tmp_tile_tex_coords.append(&mut tmp_border_right_tiles_tex_coords);

                    let upload_data_size = tmp_tile_tex_coords.len()
                        * std::mem::size_of::<GraphicTileTexureCoords>()
                        + tmp_tiles.len() * std::mem::size_of::<GraphicTile>();
                    if upload_data_size > 0 {
                        let mut upload_data_buffer =
                            graphics_mt.mem_alloc(GraphicsMemoryAllocationType::Buffer {
                                required_size: upload_data_size,
                            });

                        mem_copy_special(
                            upload_data_buffer.as_mut_slice(),
                            &tmp_tiles,
                            std::mem::size_of::<vec2>(),
                            tmp_tiles.len() * 4,
                            if do_texture_coords {
                                std::mem::size_of::<ubvec4>()
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
                                std::mem::size_of::<ubvec4>(),
                                tmp_tiles.len() * 4,
                                std::mem::size_of::<vec2>(),
                            );
                        }

                        if graphics_mt
                            .try_flush_mem(&mut upload_data_buffer, false)
                            .is_err()
                        {
                            // TODO: ignore?
                        }

                        *upload_data = (
                            visual_index,
                            Some(upload_data_buffer),
                            do_texture_coords,
                            tmp_tiles.len() as u64 * 6,
                            visuals,
                            MapRenderInfo {
                                cur_text_overlay: text_overlay_type,
                                group_index,
                                layer_index: layer_index - group.start_layer as usize,
                                visual_index,
                                is_physics_layer,
                            },
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
                let group = map.get_group(group_index);
                let layer = map.get_layer(layer_index);

                if let MapLayer::Quads(q_layer) = layer {
                    let is_textured = q_layer.0.image != -1;

                    let mut tmp_quads: Vec<TmpQuad> = Vec::new();
                    let mut tmp_quads_textured: Vec<TmpQuadTextured> = Vec::new();

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

                    let upload_data_size = if is_textured {
                        tmp_quads_textured.len() * std::mem::size_of::<TmpQuadTextured>()
                    } else {
                        tmp_quads.len() * std::mem::size_of::<TmpQuad>()
                    };

                    if upload_data_size > 0 {
                        let mut upload_data_buffer =
                            graphics_mt.mem_alloc(GraphicsMemoryAllocationType::Buffer {
                                required_size: if is_textured {
                                    tmp_quads_textured.len()
                                        * std::mem::size_of::<TmpQuadTextured>()
                                } else {
                                    tmp_quads.len() * std::mem::size_of::<TmpQuad>()
                                },
                            });

                        if is_textured {
                            tmp_quads_textured
                                .iter()
                                .enumerate()
                                .for_each(|(index, q)| {
                                    q.copy_into_slice(
                                        upload_data_buffer
                                            .as_mut_slice()
                                            .split_at_mut(
                                                index * std::mem::size_of::<TmpQuadTextured>(),
                                            )
                                            .1,
                                        0,
                                    )
                                });

                            upload_data_buffer
                                .as_mut_slice()
                                .par_chunks_exact_mut(std::mem::size_of::<TmpQuadTextured>())
                                .enumerate()
                                .for_each(|(i, q_slice)| {
                                    tmp_quads_textured[i].copy_into_slice(q_slice, 0)
                                });
                        } else {
                            upload_data_buffer
                                .as_mut_slice()
                                .par_chunks_exact_mut(std::mem::size_of::<TmpQuad>())
                                .enumerate()
                                .for_each(|(i, q_slice)| tmp_quads[i].copy_into_slice(q_slice, 0));
                        }

                        if graphics_mt
                            .try_flush_mem(&mut upload_data_buffer, false)
                            .is_err()
                        {
                            // TODO: ignore?
                        }

                        *upload_data = (
                            visual_index,
                            Some(upload_data_buffer),
                            is_textured,
                            (q_layer.0.num_quads * 6) as u64,
                            q_layer_visuals,
                            MapRenderInfo {
                                visual_index,
                                group_index,
                                layer_index: layer_index - group.start_layer as usize,
                                cur_text_overlay: None,
                                is_physics_layer: false,
                            },
                        );
                    }
                }
            });

        upload_data
    }

    pub fn upload_map(
        &mut self,
        graphics: &mut Graphics,
        mut upload_data: ClientMapBufferUploadData,
    ) {
        let mut tile_render_infos: Vec<MapRenderInfo> = Default::default();
        let mut quad_render_infos: Vec<MapRenderInfo> = Default::default();

        tile_render_infos.reserve(upload_data.tile_layer_uploads.len());
        quad_render_infos.reserve(upload_data.quad_layer_uploads.len());

        upload_data.tile_layer_uploads.drain(..).for_each(
            |(_, raw_data, _, indices_count, visuals, render_info)| {
                if raw_data.is_none() || raw_data.as_ref().unwrap().as_slice().is_empty() {
                    self.visuals.tile_layer_visuals.push(TileLayerVisuals {
                        base: visuals,
                        buffer_object_index: None,
                    });
                } else {
                    // create the buffer object
                    self.visuals.tile_layer_visuals.push(TileLayerVisuals {
                        base: visuals,
                        buffer_object_index: Some(
                            graphics
                                .buffer_object_handle
                                .create_buffer_object(raw_data.unwrap()),
                        ),
                    });
                    tile_render_infos.push(render_info);
                    // and finally inform the backend how many indices are required
                    graphics.indices_num_required_notify(indices_count);
                }
            },
        );

        upload_data.quad_layer_uploads.drain(..).for_each(
            |(_, raw_data, _, indices_count, visuals, render_info)| {
                if raw_data.is_none() || raw_data.as_ref().unwrap().as_slice().is_empty() {
                    self.visuals.quad_layer_visuals.push(QuadLayerVisuals {
                        base: visuals,
                        buffer_object_index: None,
                    });
                } else {
                    // create the buffer object
                    self.visuals.quad_layer_visuals.push(QuadLayerVisuals {
                        base: visuals,
                        buffer_object_index: Some(
                            graphics
                                .buffer_object_handle
                                .create_buffer_object(raw_data.unwrap()),
                        ),
                    });
                    quad_render_infos.push(render_info);
                    // and finally inform the backend how many indices are required
                    graphics.indices_num_required_notify(indices_count);
                }
            },
        );

        let mut background_render_layers: Vec<MapRenderLayer> = Default::default();
        background_render_layers.reserve(tile_render_infos.len() + quad_render_infos.len());
        let mut foreground_render_layers: Vec<MapRenderLayer> = Default::default();
        foreground_render_layers.reserve(tile_render_infos.len() + quad_render_infos.len());
        let mut tile_index = 0;
        let mut quad_index = 0;
        while tile_index < tile_render_infos.len() || quad_index < quad_render_infos.len() {
            let render_info_tile = if tile_index < tile_render_infos.len() {
                tile_render_infos[tile_index]
            } else {
                MapRenderInfo {
                    cur_text_overlay: None,
                    group_index: usize::MAX,
                    layer_index: usize::MAX,
                    visual_index: usize::MAX,
                    is_physics_layer: false,
                }
            };
            let render_info_quad = if quad_index < quad_render_infos.len() {
                quad_render_infos[quad_index]
            } else {
                MapRenderInfo {
                    cur_text_overlay: None,
                    group_index: usize::MAX,
                    layer_index: usize::MAX,
                    visual_index: usize::MAX,
                    is_physics_layer: false,
                }
            };
            let mut push_render_infos =
                |render_layer: MapRenderLayer, group_index: usize, layer_index: usize| {
                    if group_index < upload_data.main_physics_layer_group_index
                        || (group_index == upload_data.main_physics_layer_group_index
                            && layer_index < upload_data.main_physics_layer_layer_index)
                    {
                        background_render_layers.push(render_layer);
                    } else {
                        foreground_render_layers.push(render_layer);
                    }
                };
            if render_info_tile.group_index < render_info_quad.group_index {
                push_render_infos(
                    MapRenderLayer::Tile(tile_render_infos[tile_index]),
                    render_info_tile.group_index,
                    render_info_tile.layer_index,
                );
                tile_index += 1;
            } else if render_info_tile.group_index == render_info_quad.group_index {
                if render_info_tile.layer_index < render_info_quad.layer_index {
                    push_render_infos(
                        MapRenderLayer::Tile(tile_render_infos[tile_index]),
                        render_info_tile.group_index,
                        render_info_tile.layer_index,
                    );
                    tile_index += 1;
                } else {
                    push_render_infos(
                        MapRenderLayer::Quad(quad_render_infos[quad_index]),
                        quad_render_infos[quad_index].group_index,
                        quad_render_infos[quad_index].layer_index,
                    );
                    quad_index += 1;
                }
            } else {
                push_render_infos(
                    MapRenderLayer::Quad(quad_render_infos[quad_index]),
                    quad_render_infos[quad_index].group_index,
                    quad_render_infos[quad_index].layer_index,
                );
                quad_index += 1;
            }
        }

        self.render.background_render_layers = background_render_layers;
        self.render.foreground_render_layers = foreground_render_layers;

        self.info.main_physics_layer_group_index = upload_data.main_physics_layer_group_index;
        self.info.main_physics_layer_layer_index = upload_data.main_physics_layer_layer_index;
    }
}
