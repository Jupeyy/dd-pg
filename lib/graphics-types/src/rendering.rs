use bincode::{Decode, Encode};
use math::math::vector::{vec2, vec4_base};

#[derive(Default, Copy, Clone, PartialEq, Eq, Encode, Decode)]
pub enum BlendType {
    BLEND_NONE = 0,
    #[default]
    BLEND_ALPHA,
    BLEND_ADDITIVE,
}

#[derive(Default, Copy, Clone, PartialEq, Eq, Encode, Decode)]
pub enum WrapType {
    #[default]
    WRAP_REPEAT = 0,
    WRAP_CLAMP,
}

pub type SPoint = vec2;

#[derive(Copy, Clone, Encode, Decode)]
pub enum ETextureIndex {
    Index(usize),
    Invalid,
}

impl Default for ETextureIndex {
    fn default() -> Self {
        Self::Invalid
    }
}

impl ETextureIndex {
    pub fn unwrap(&self) -> usize {
        if let ETextureIndex::Index(index) = self {
            return *index;
        }
        panic!("invalid texture index.");
    }

    pub fn is_invalid(&self) -> bool {
        if let ETextureIndex::Invalid = self {
            return true;
        }
        false
    }
}

#[derive(Copy, Clone, Encode, Decode)]
pub struct State {
    pub blend_mode: BlendType,
    pub wrap_mode: WrapType,
    pub texture_index: ETextureIndex,
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
            texture_index: ETextureIndex::Invalid,
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
        self.texture_index = ETextureIndex::Invalid;
    }

    pub fn set_texture(&mut self, tex_index: ETextureIndex) {
        self.texture_index = tex_index;
    }

    pub fn blend_none(&mut self) {
        self.blend_mode = BlendType::BLEND_NONE;
    }

    pub fn blend_normal(&mut self) {
        self.blend_mode = BlendType::BLEND_ALPHA;
    }

    pub fn blend_additive(&mut self) {
        self.blend_mode = BlendType::BLEND_ADDITIVE;
    }

    pub fn wrap_clamp(&mut self) {
        self.wrap_mode = WrapType::WRAP_CLAMP;
    }

    pub fn wrap_normal(&mut self) {
        self.wrap_mode = WrapType::WRAP_REPEAT;
    }

    pub fn clip(&mut self, x: i32, y: i32, w: u32, h: u32) {
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

    pub fn get_canvas_mapping(
        &self,
        top_left_x: &mut f32,
        top_left_y: &mut f32,
        bottom_right_x: &mut f32,
        bottom_right_y: &mut f32,
    ) {
        *bottom_right_x = self.canvas_br.x;
        *bottom_right_y = self.canvas_br.y;
        *top_left_x = self.canvas_tl.x;
        *top_left_y = self.canvas_tl.y;
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

pub type GL_SColorf = ColorRGBA;
//use normalized color values
pub type GL_SColor = vec4_base<u8>;

pub type GL_SPoint = vec2;
pub type GL_STexCoord = vec2;

pub trait WriteVertexAttributes {
    fn set_pos(&mut self, pos: &GL_SPoint);
    fn get_pos(&self) -> &GL_SPoint;
    fn set_tex_coords(&mut self, coords: &GL_STexCoord);
    fn set_color(&mut self, color: &GL_SColor);
}

#[repr(C)]
#[derive(Copy, Clone, Default, Encode, Decode)]
pub struct GL_SVertex {
    pub pos: GL_SPoint,
    pub tex: GL_STexCoord,
    pub color: GL_SColor,
}

impl GL_SVertex {
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

impl WriteVertexAttributes for GL_SVertex {
    fn set_pos(&mut self, pos: &GL_SPoint) {
        self.pos = *pos;
    }
    fn get_pos(&self) -> &GL_SPoint {
        &self.pos
    }
    fn set_tex_coords(&mut self, coords: &GL_STexCoord) {
        self.tex = *coords;
    }
    fn set_color(&mut self, color: &GL_SColor) {
        self.color = *color;
    }
}

pub type SVertex = GL_SVertex;
