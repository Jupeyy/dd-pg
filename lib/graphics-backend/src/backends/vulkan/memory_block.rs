use std::sync::Arc;

use ash::vk;
use hiarc::Hiarc;

use super::{
    common::verbose_deallocated_memory, frame_resources::FrameResources,
    logical_device::LogicalDevice, vulkan_dbg::is_verbose, vulkan_types::EMemoryBlockUsage,
};

#[derive(Debug, Clone, Hiarc)]
pub struct DeviceMemoryBlock {
    #[hiarc_skip_unsafe]
    mem: vk::DeviceMemory,
    size: vk::DeviceSize,
    usage_type: EMemoryBlockUsage,

    device: Arc<LogicalDevice>,
}

impl DeviceMemoryBlock {
    pub fn new(
        device: Arc<LogicalDevice>,
        mem_alloc_info: vk::MemoryAllocateInfo,
        usage_type: EMemoryBlockUsage,
    ) -> anyhow::Result<Arc<Self>, vk::Result> {
        let size = mem_alloc_info.allocation_size;
        match usage_type {
            EMemoryBlockUsage::Texture => {
                device
                    .texture_memory_usage
                    .fetch_add(size, std::sync::atomic::Ordering::Relaxed);
            }
            EMemoryBlockUsage::Buffer => {
                device
                    .buffer_memory_usage
                    .fetch_add(size, std::sync::atomic::Ordering::Relaxed);
            }
            EMemoryBlockUsage::Stream => {
                device
                    .stream_memory_usage
                    .fetch_add(size, std::sync::atomic::Ordering::Relaxed);
            }
            EMemoryBlockUsage::Staging => {
                device
                    .staging_memory_usage
                    .fetch_add(size, std::sync::atomic::Ordering::Relaxed);
            }
            EMemoryBlockUsage::Dummy => {}
        };

        let mem = unsafe { device.device.allocate_memory(&mem_alloc_info, None) }?;

        Ok(Arc::new(Self {
            device,

            mem,
            size,
            usage_type,
        }))
    }

    pub fn size(&self) -> vk::DeviceSize {
        self.size
    }

    pub fn mem(self: &Arc<Self>, frame_resources: &mut FrameResources) -> vk::DeviceMemory {
        frame_resources.device_memory.push(self.clone());

        self.mem
    }
}

impl Drop for DeviceMemoryBlock {
    fn drop(&mut self) {
        match self.usage_type {
            EMemoryBlockUsage::Texture => {
                self.device
                    .texture_memory_usage
                    .fetch_sub(self.size, std::sync::atomic::Ordering::Relaxed);
            }
            EMemoryBlockUsage::Buffer => {
                self.device
                    .buffer_memory_usage
                    .fetch_sub(self.size, std::sync::atomic::Ordering::Relaxed);
            }
            EMemoryBlockUsage::Stream => {
                self.device
                    .stream_memory_usage
                    .fetch_sub(self.size, std::sync::atomic::Ordering::Relaxed);
            }
            EMemoryBlockUsage::Staging => {
                self.device
                    .staging_memory_usage
                    .fetch_sub(self.size, std::sync::atomic::Ordering::Relaxed);
            }
            EMemoryBlockUsage::Dummy => {}
        };

        if is_verbose(&self.device.dbg) {
            verbose_deallocated_memory(self.size, self.usage_type);
        }

        unsafe {
            self.device.device.free_memory(self.mem, None);
        }
    }
}
