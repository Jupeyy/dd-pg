use std::ops::{Index, IndexMut};

use map::map::groups::layers::tiles::{rotation_180, rotation_270, TileFlags, ROTATION_90};
use math::math::vector::{ivec2, vec2};

use super::graphic_tile::GraphicsTileTex;

pub(super) type GraphicsBorderTilePos = vec2;
pub(super) type GraphicsBorderTileTex = GraphicsTileTex;

#[repr(C)]
#[derive(Default)]
pub(super) struct GraphicBorderTile {
    top_left: GraphicsBorderTilePos,
    tex_coord_top_left: GraphicsBorderTileTex,
    top_right: GraphicsBorderTilePos,
    tex_coord_top_right: GraphicsBorderTileTex,
    bottom_right: GraphicsBorderTilePos,
    tex_coord_bottom_right: GraphicsBorderTileTex,
    bottom_left: GraphicsBorderTilePos,
    tex_coord_bottom_left: GraphicsBorderTileTex,
}

impl Index<usize> for GraphicBorderTile {
    type Output = GraphicsBorderTilePos;

    fn index(&self, index: usize) -> &GraphicsBorderTilePos {
        match index {
            0 => &self.top_left,
            1 => &self.top_right,
            2 => &self.bottom_right,
            3 => &self.bottom_left,
            _ => panic!("index out of bounds"),
        }
    }
}

impl IndexMut<usize> for GraphicBorderTile {
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

impl GraphicBorderTile {
    pub(super) fn copy_into_slice(&self, dest: &mut [u8], textured: bool) -> usize {
        fn copy_pos_into_slice(pos: &GraphicsBorderTilePos, dest: &mut [u8]) -> usize {
            let mut off: usize = 0;

            pos.x.to_ne_bytes().iter().for_each(|byte| {
                dest[off] = *byte;
                off += 1;
            });
            pos.y.to_ne_bytes().iter().for_each(|byte| {
                dest[off] = *byte;
                off += 1;
            });
            off
        }
        fn copy_tex_into_slice(tex: &GraphicsBorderTileTex, dest: &mut [u8]) -> usize {
            let mut off: usize = 0;

            tex.x.to_ne_bytes().iter().for_each(|byte| {
                dest[off] = *byte;
                off += 1;
            });
            tex.y.to_ne_bytes().iter().for_each(|byte| {
                dest[off] = *byte;
                off += 1;
            });
            tex.z.to_ne_bytes().iter().for_each(|byte| {
                dest[off] = *byte;
                off += 1;
            });
            tex.w.to_ne_bytes().iter().for_each(|byte| {
                dest[off] = *byte;
                off += 1;
            });
            off
        }
        let mut off = 0;
        for index in 0..4 {
            off += copy_pos_into_slice(
                match index {
                    0 => &self.top_left,
                    1 => &self.top_right,
                    2 => &self.bottom_right,
                    3 => &self.bottom_left,
                    _ => panic!("out of bounds"),
                },
                &mut dest[off..],
            );
            if textured {
                off += copy_tex_into_slice(
                    match index {
                        0 => &self.tex_coord_top_left,
                        1 => &self.tex_coord_top_right,
                        2 => &self.tex_coord_bottom_right,
                        3 => &self.tex_coord_bottom_left,
                        _ => panic!("out of bounds"),
                    },
                    &mut dest[off..],
                );
            }
        }
        off
    }
}

fn fill_tmp_tile_speedup(
    tmp_tile: &mut GraphicBorderTile,
    _flags: TileFlags,
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
        if angle >= 270 {
            rotation_270()
        } else {
            if angle >= 180 {
                rotation_180()
            } else {
                if angle >= 90 {
                    ROTATION_90
                } else {
                    TileFlags::empty()
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
    tmp_tile: &mut GraphicBorderTile,
    flags: TileFlags,
    index: u8,
    x: i32,
    y: i32,
    offset: &ivec2,
    scale: i32,
) {
    // tile tex
    let mut x0: u8 = 0;
    let mut y0: u8 = 0;
    let mut x1: u8 = x0 + 1;
    let mut y1: u8 = y0;
    let mut x2: u8 = x0 + 1;
    let mut y2: u8 = y0 + 1;
    let mut x3: u8 = x0;
    let mut y3: u8 = y0 + 1;

    if !(flags & TileFlags::XFLIP).is_empty() {
        x0 = x2;
        x1 = x3;
        x2 = x3;
        x3 = x0;
    }

    if !(flags & TileFlags::YFLIP).is_empty() {
        y0 = y3;
        y2 = y1;
        y3 = y1;
        y1 = y0;
    }

    if !(flags & TileFlags::ROTATE).is_empty() {
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

    tmp_tile.tex_coord_top_left.x = x0;
    tmp_tile.tex_coord_top_left.y = y0;
    tmp_tile.tex_coord_bottom_left.x = x3;
    tmp_tile.tex_coord_bottom_left.y = y3;
    tmp_tile.tex_coord_top_right.x = x1;
    tmp_tile.tex_coord_top_right.y = y1;
    tmp_tile.tex_coord_bottom_right.x = x2;
    tmp_tile.tex_coord_bottom_right.y = y2;

    tmp_tile.tex_coord_top_left.z = index;
    tmp_tile.tex_coord_bottom_left.z = index;
    tmp_tile.tex_coord_top_right.z = index;
    tmp_tile.tex_coord_bottom_right.z = index;

    let has_rotation = !(flags & TileFlags::ROTATE).is_empty();
    tmp_tile.tex_coord_top_left.w = has_rotation as u8;
    tmp_tile.tex_coord_bottom_left.w = has_rotation as u8;
    tmp_tile.tex_coord_top_right.w = has_rotation as u8;
    tmp_tile.tex_coord_bottom_right.w = has_rotation as u8;

    // tile pos
    tmp_tile.top_left.x = (x * scale) as f32 + offset.x as f32;
    tmp_tile.top_left.y = (y * scale) as f32 + offset.y as f32;
    tmp_tile.bottom_left.x = (x * scale) as f32 + offset.x as f32;
    tmp_tile.bottom_left.y = (y * scale + scale) as f32 + offset.y as f32;
    tmp_tile.top_right.x = (x * scale + scale) as f32 + offset.x as f32;
    tmp_tile.top_right.y = (y * scale) as f32 + offset.y as f32;
    tmp_tile.bottom_right.x = (x * scale + scale) as f32 + offset.x as f32;
    tmp_tile.bottom_right.y = (y * scale + scale) as f32 + offset.y as f32;
}

pub(super) fn add_border_tile(
    tmp_tiles: &mut Vec<GraphicBorderTile>,
    index: u8,
    flags: TileFlags,
    x: i32,
    y: i32,
    fill_speedup: bool,
    angle_rotate: i16,
    offset: &ivec2,
    ignore_index_check: bool,
) -> bool {
    if index > 0 || ignore_index_check {
        let mut tile = GraphicBorderTile::default();
        if fill_speedup {
            fill_tmp_tile_speedup(&mut tile, flags, 0, x, y, offset, 1, angle_rotate as i16);
        } else {
            fill_tmp_tile(&mut tile, flags, index, x, y, offset, 1);
        }
        tmp_tiles.push(tile);

        return true;
    }
    return false;
}
