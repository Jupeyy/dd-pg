/*******************************
 * UNIFORM PUSH CONSTANT LAYOUTS
 ********************************/

use graphics_types::rendering::ColorRGBA;
use math::math::vector::{vec2, vec4};

#[derive(Default)]
#[repr(C)]
pub struct UniformGPos {
    pub pos: [f32; 4 * 2],
}

#[derive(Default)]
#[repr(C)]
pub struct UniformGBlur {
    pub texture_size: vec2,
    pub scale: vec2,
    pub color: vec4,
    pub blur_radius: f32,
}

#[derive(Default)]
#[repr(C)]
pub struct UniformPrimExGPosRotationless {
    pub pos: [f32; 4 * 2],
}

#[derive(Default)]
#[repr(C)]
pub struct UniformPrimExGPos {
    pub base: UniformPrimExGPosRotationless,
    pub center: vec2,
    pub rotation: f32,
}

pub type SUniformPrimExGVertColor = ColorRGBA;

#[derive(Default)]
#[repr(C)]
pub struct UniformPrimExGVertColorAlign {
    pub pad: [f32; (48 - 44) / 4],
}

#[derive(Default)]
#[repr(C)]
pub struct UniformSpriteMultiGPos {
    pub pos: [f32; 4 * 2],
    pub center: vec2,
}

pub type SUniformSpriteMultiGVertColor = ColorRGBA;

#[derive(Default)]
#[repr(C)]
pub struct UniformSpriteMultiGVertColorAlign {
    pub pad: [f32; (48 - 40) / 4],
}
