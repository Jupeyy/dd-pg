/*******************************
 * UNIFORM PUSH CONSTANT LAYOUTS
 ********************************/

use graphics_types::rendering::ColorRGBA;
use math::math::vector::{vec2, vec3, vec4};

#[derive(Default)]
#[repr(C)]
pub struct SUniformGPos {
    pub pos: [f32; 4 * 2],
}

#[derive(Default)]
#[repr(C)]
pub struct SUniformGTextPos {
    pub pos: [f32; 4 * 2],
    pub texture_size: f32,
}

#[derive(Default)]
#[repr(C)]
pub struct SUniformGBlur {
    pub texture_size: vec2,
    pub blur_radius: f32,
}

pub type SUniformTextGFragmentOffset = vec3;

#[derive(Default)]
#[repr(C)]
pub struct SUniformTextGFragmentConstants {
    pub text_color: ColorRGBA,
    pub text_outline_color: ColorRGBA,
}

#[derive(Default)]
#[repr(C)]
pub struct SUniformTextFragment {
    pub constants: SUniformTextGFragmentConstants,
}

#[derive(Default)]
#[repr(C)]
pub struct SUniformTileGPos {
    pub pos: [f32; 4 * 2],
}

#[derive(Default)]
#[repr(C)]
pub struct SUniformTileGPosBorderLine {
    pub base: SUniformTileGPos,
    pub dir: vec2,
    pub offset: vec2,
}

#[derive(Default)]
#[repr(C)]
pub struct SUniformTileGPosBorder {
    pub base: SUniformTileGPosBorderLine,
    pub jump_index: i32,
}

pub type SUniformTileGVertColor = ColorRGBA;

#[derive(Default)]
#[repr(C)]
pub struct SUniformTileGVertColorAlign {
    pub pad: [f32; (64 - 52) / 4],
}

#[derive(Default)]
#[repr(C)]
pub struct SUniformPrimExGPosRotationless {
    pub pos: [f32; 4 * 2],
}

#[derive(Default)]
#[repr(C)]
pub struct SUniformPrimExGPos {
    pub base: SUniformPrimExGPosRotationless,
    pub center: vec2,
    pub rotation: f32,
}

pub type SUniformPrimExGVertColor = ColorRGBA;

#[derive(Default)]
#[repr(C)]
pub struct SUniformPrimExGVertColorAlign {
    pub pad: [f32; (48 - 44) / 4],
}

#[derive(Default)]
#[repr(C)]
pub struct SUniformSpriteMultiGPos {
    pub pos: [f32; 4 * 2],
    pub center: vec2,
}

pub type SUniformSpriteMultiGVertColor = ColorRGBA;

#[derive(Default)]
#[repr(C)]
pub struct SUniformSpriteMultiGVertColorAlign {
    pub pad: [f32; (48 - 40) / 4],
}

#[derive(Default)]
#[repr(C)]
pub struct SUniformSpriteMultiPushGPosBase {
    pub pos: [f32; 4 * 2],
    pub center: vec2,
    pub padding: vec2,
}

#[derive(Default)]
#[repr(C)]
pub struct SUniformSpriteMultiPushGPos {
    pub base: SUniformSpriteMultiPushGPosBase,
    pub psr: [vec4; 1],
}

pub type SUniformSpriteMultiPushGVertColor = ColorRGBA;

#[derive(Default)]
#[repr(C)]
pub struct SUniformQuadGPosBase {
    pub pos: [f32; 4 * 2],
    pub quad_offset: i32,
}

#[derive(Default)]
#[repr(C)]
pub struct SUniformQuadPushGBufferObject {
    pub vert_color: vec4,
    pub offset: vec2,
    pub rotation: f32,
    pub padding: f32,
}

#[derive(Default)]
#[repr(C)]
pub struct SUniformQuadPushGPos {
    pub pos: [f32; 4 * 2],
    pub bo_push: SUniformQuadPushGBufferObject,
    pub quad_offset: i32,
}

#[derive(Default)]
#[repr(C)]
pub struct SUniformQuadGPos {
    pub pos: [f32; 4 * 2],
    pub quad_offset: i32,
}
