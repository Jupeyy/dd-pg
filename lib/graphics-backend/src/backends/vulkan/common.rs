use ash::vk;
use graphics_types::commands::TexFormat;
use hiarc::Hiarc;

use super::vulkan_types::EMemoryBlockUsage;

#[derive(Debug, Hiarc, Default, Clone)]
pub struct GraphicsGpuItem {
    pub name: String,
    pub gpu_type: u32, // @see ETWGraphicsGPUType
}

#[derive(Debug, Hiarc, Default, Clone)]
pub struct GraphicsGpus {
    pub gpus: Vec<GraphicsGpuItem>,
    pub auto_gpu: GraphicsGpuItem,
}

pub type TTWGraphicsGPUList = GraphicsGpus;

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
