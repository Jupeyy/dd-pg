use std::collections::VecDeque;

use ash::vk;
use graphics_types::command_buffer::TexFormat;

use super::vulkan_types::EMemoryBlockUsage;

#[derive(Debug, Default, Clone)]
pub struct STWGraphicGPUItem {
    pub name: String,
    pub gpu_type: u32, // @see ETWGraphicsGPUType
}

#[derive(Debug, Default)]
pub struct STWGraphicGPU {
    pub gpus: Vec<STWGraphicGPUItem>,
    pub auto_gpu: STWGraphicGPUItem,
}

pub type TTWGraphicsGPUList = STWGraphicGPU;

#[derive(Debug)]
pub enum EGFXErrorType {
    None = 0,
    Init,
    OutOfMemoryImage,
    OutOfMemoryBuffer,
    OutOfMemoryStaging,
    RenderRecording,
    RenderSubmitFailed,
    SwapFailed,
}

#[derive(Debug)]
pub enum EGFXWarningType {
    None = 0,
    InitFailed,
    _LowOnMemory,
    MissingExtension,
    _InitFailedMissingIntegratedGPUDriver,
}

#[derive(Debug)]
pub struct SGFXErrorContainer {
    pub error_type: EGFXErrorType,
    pub errors: Vec<String>,
}

impl Default for SGFXErrorContainer {
    fn default() -> Self {
        Self {
            error_type: EGFXErrorType::None,
            errors: Default::default(),
        }
    }
}

#[derive(Debug)]
pub struct SGFXWarningContainer {
    pub warning_type: EGFXWarningType, // TODO = EGFXWarningType::GFX_WARNING_TYPE_NONE;
    pub warnings: VecDeque<String>,
}

impl Default for SGFXWarningContainer {
    fn default() -> Self {
        Self {
            warning_type: EGFXWarningType::None,
            warnings: Default::default(),
        }
    }
}

pub fn localizable(in_str: &'static str) -> &'static str {
    in_str
}

pub fn verbose_allocated_memory(_size: vk::DeviceSize, mem_usage: EMemoryBlockUsage) {
    let mut _usage_str = "unknown";
    match mem_usage {
        EMemoryBlockUsage::Texture => _usage_str = "texture",
        EMemoryBlockUsage::Buffer => _usage_str = "buffer",
        EMemoryBlockUsage::Stream => _usage_str = "stream",
        EMemoryBlockUsage::Staging => _usage_str = "staging buffer",
        _ => (),
    }
    // TODO dbg_msg("vulkan", "allocated chunk of memory with size: %" PRIu64 " for frame %" PRIu64 " (%s)", (usize)Size, (usize)m_CurImageIndex, pUsage);
}

pub fn verbose_deallocated_memory(_size: vk::DeviceSize, mem_usage: EMemoryBlockUsage) {
    let mut _usage_str = "unknown";
    match mem_usage {
        EMemoryBlockUsage::Texture => _usage_str = "texture",
        EMemoryBlockUsage::Buffer => _usage_str = "buffer",
        EMemoryBlockUsage::Stream => _usage_str = "stream",
        EMemoryBlockUsage::Staging => _usage_str = "staging buffer",
        _ => (),
    }
    // TODO dbg_msg("vulkan", "deallocated chunk of memory with size: %" PRIu64 " for frame %" PRIu64 " (%s)", (usize)Size, (usize)m_CurImageIndex, pUsage);
}

pub fn image_mip_level_count_ex(width: usize, height: usize, depth: usize) -> usize {
    (((std::cmp::max(width, std::cmp::max(height, depth)) as f32).log2()).floor() + 1.0) as usize
}

pub fn image_mip_level_count(img_extent: vk::Extent3D) -> usize {
    image_mip_level_count_ex(
        img_extent.width as usize,
        img_extent.height as usize,
        img_extent.depth as usize,
    )
}

pub fn _vulkan_format_to_image_color_channel_count(format: vk::Format) -> usize {
    if format == vk::Format::R8G8B8_UNORM {
        return 3;
    } else if format == vk::Format::R8G8B8A8_UNORM {
        return 4;
    } else if format == vk::Format::R8_UNORM {
        return 1;
    }
    4
}

pub fn texture_format_to_vulkan_format(tex_format: i32) -> vk::Format {
    if tex_format == TexFormat::RGBA as i32 {
        return vk::Format::R8G8B8A8_UNORM;
    }
    vk::Format::R8G8B8A8_UNORM
}

pub fn tex_format_to_image_color_channel_count(tex_format: i32) -> usize {
    if tex_format == TexFormat::RGBA as i32 {
        return 4;
    }
    4
}
