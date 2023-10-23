use config::config::{ConfigBackend, ConfigDebug};

pub mod barriers;
pub mod buffer;
pub mod command_buffer;
pub mod command_pool;
pub mod common;
pub mod descriptor_layout;
pub mod descriptor_pool;
pub mod descriptor_set;
pub mod fence;
pub mod frame;
pub mod image;
pub mod image_view;
pub mod instance;
pub mod logical_device;
pub mod mapped_memory;
pub mod memory;
pub mod memory_allocator;
pub mod memory_block;
pub mod phy_device;
pub mod queue;
pub mod render_pass;
pub mod semaphore;
pub mod streamed_uniform;
pub mod utils;
pub mod vulkan;
pub mod vulkan_allocator;
pub mod vulkan_config;
pub mod vulkan_dbg;
pub mod vulkan_device;
pub mod vulkan_error;
pub mod vulkan_limits;
pub mod vulkan_mem;
pub mod vulkan_types;
pub mod vulkan_uniform;

pub struct Options<'a> {
    pub dbg: &'a ConfigDebug,
    pub gl: &'a ConfigBackend,
}
