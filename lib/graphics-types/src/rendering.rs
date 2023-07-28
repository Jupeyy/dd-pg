use std::rc::Rc;

use base::counted_index::{CountedIndex, CountedIndexGetIndexUnsafe};
use bincode::{Decode, Encode};
use math::math::vector::{vec2, vec4_base};

#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, Encode, Decode)]
pub enum BlendType {
    None = 0,
    #[default]
    Alpha,
    Additive,
}

#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, Encode, Decode)]
pub enum WrapType {
    #[default]
    Repeat = 0,
    Clamp,
}

pub type SPoint = vec2;

pub type TextureIndex = CountedIndex<true>;

#[derive(Debug, Copy, Clone, Encode, Decode)]
pub struct State {
    pub blend_mode: BlendType,
    pub wrap_mode: WrapType,
    pub texture_index: Option<usize>,
    pub canvas_tl: SPoint,
    pub canvas_br: SPoint,

    // clip
    pub clip_enable: bool,
    pub clip_x: i32,
    pub clip_y: i32,
    pub clip_w: u32,
    pub clip_h: u32,
}

impl State {
    pub fn new() -> State {
        State {
            blend_mode: Default::default(),
            wrap_mode: Default::default(),
            texture_index: None,
            canvas_tl: SPoint::default(),
            canvas_br: SPoint::default(),

            // clip
            clip_enable: false,
            clip_x: 0,
            clip_y: 0,
            clip_w: 0,
            clip_h: 0,
        }
    }

    pub fn clear_texture(&mut self) {
        self.texture_index = None;
    }

    pub fn set_texture(&mut self, tex_index: &dyn CountedIndexGetIndexUnsafe) {
        self.texture_index = Some(tex_index.get_index_unsafe());
    }

    pub fn blend_none(&mut self) {
        self.blend_mode = BlendType::None;
    }

    pub fn blend_normal(&mut self) {
        self.blend_mode = BlendType::Alpha;
    }

    pub fn blend_additive(&mut self) {
        self.blend_mode = BlendType::Additive;
    }

    pub fn wrap_clamp(&mut self) {
        self.wrap_mode = WrapType::Clamp;
    }

    pub fn wrap_normal(&mut self) {
        self.wrap_mode = WrapType::Repeat;
    }

    pub fn clip_clamped(
        &mut self,
        x: i32,
        y: i32,
        mut w: u32,
        mut h: u32,
        canvas_w: u32,
        canvas_h: u32,
    ) {
        if x < 0 {
            w = (w as i64 + (x as i64).max(-(w as i64))) as u32;
        }

        if y < 0 {
            h = (h as i64 + (y as i64).max(-(h as i64))) as u32;
        }

        self.clip_x = x.clamp(0, canvas_w as i32);
        self.clip_y = y.clamp(0, canvas_h as i32);
        self.clip_w = w.clamp(0, canvas_w - self.clip_x as u32);
        self.clip_h = h.clamp(0, canvas_h - self.clip_y as u32);

        self.clip_y = canvas_h as i32 - (self.clip_y + self.clip_h as i32);
    }

    pub fn clip(&mut self, x: i32, y: i32, mut w: u32, mut h: u32) {
        self.clip_enable = true;
        self.clip_x = x;
        self.clip_y = y;
        self.clip_w = w;
        self.clip_h = h;
    }

    pub fn map_canvas(
        &mut self,
        top_left_x: f32,
        top_left_y: f32,
        bottom_right_x: f32,
        bottom_right_y: f32,
    ) {
        self.canvas_br.x = bottom_right_x;
        self.canvas_br.y = bottom_right_y;
        self.canvas_tl.x = top_left_x;
        self.canvas_tl.y = top_left_y;
    }

    pub fn get_canvas_mapping(&self) -> (f32, f32, f32, f32) {
        (
            self.canvas_tl.x,
            self.canvas_tl.y,
            self.canvas_br.x,
            self.canvas_br.y,
        )
    }

    pub fn get_canvas_width(&self) -> f32 {
        self.canvas_br.x - self.canvas_tl.x
    }

    pub fn get_canvas_height(&self) -> f32 {
        self.canvas_br.y - self.canvas_tl.y
    }
}

impl Default for State {
    fn default() -> Self {
        Self::new()
    }
}

#[repr(C)]
#[derive(Default, Copy, Clone)]
pub struct ColorRGBA {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl ColorRGBA {
    pub fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }
}

pub type GlColorf = ColorRGBA;
//use normalized color values
pub type GlColor = vec4_base<u8>;

pub type GlPoint = vec2;
pub type GlTexCoord = vec2;

pub trait WriteVertexAttributes {
    fn set_pos(&mut self, pos: &GlPoint);
    fn get_pos(&self) -> &GlPoint;
    fn set_tex_coords(&mut self, coords: &GlTexCoord);
    fn set_color(&mut self, color: &GlColor);
}

#[repr(C)]
#[derive(Copy, Clone, Default, Encode, Decode)]
pub struct GlVertex {
    pub pos: GlPoint,
    pub tex: GlTexCoord,
    pub color: GlColor,
}

impl GlVertex {
    pub fn append_to_bytes_vec(&self, bytes_vec: &mut Vec<u8>) {
        let bytes = self.pos.x.to_ne_bytes();
        bytes.iter().for_each(|byte| {
            bytes_vec.push(*byte);
        });
        let bytes = self.pos.y.to_ne_bytes();
        bytes.iter().for_each(|byte| {
            bytes_vec.push(*byte);
        });
        let bytes = self.tex.x.to_ne_bytes();
        bytes.iter().for_each(|byte| {
            bytes_vec.push(*byte);
        });
        let bytes = self.tex.y.to_ne_bytes();
        bytes.iter().for_each(|byte| {
            bytes_vec.push(*byte);
        });
        bytes_vec.push(self.color.r());
        bytes_vec.push(self.color.g());
        bytes_vec.push(self.color.b());
        bytes_vec.push(self.color.a());
    }
}

impl WriteVertexAttributes for GlVertex {
    fn set_pos(&mut self, pos: &GlPoint) {
        self.pos = *pos;
    }
    fn get_pos(&self) -> &GlPoint {
        &self.pos
    }
    fn set_tex_coords(&mut self, coords: &GlTexCoord) {
        self.tex = *coords;
    }
    fn set_color(&mut self, color: &GlColor) {
        self.color = *color;
    }
}

pub type SVertex = GlVertex;
