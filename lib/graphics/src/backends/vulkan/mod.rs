use base::config::EDebugGFXModes;

pub mod common;
pub mod vulkan;
pub mod vulkan_allocator;
pub mod vulkan_dbg;
pub mod vulkan_device;
pub mod vulkan_error;
pub mod vulkan_limits;
pub mod vulkan_mem;
pub mod vulkan_types;
pub mod vulkan_uniform;

pub struct Options {
    pub thread_count: usize,
    pub dbg_gfx: EDebugGFXModes,
}
