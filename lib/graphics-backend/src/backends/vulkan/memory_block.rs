use std::sync::Arc;

use ash::vk;

use super::{logical_device::LogicalDevice, vulkan_types::EMemoryBlockUsage};

#[derive(Debug, Clone)]
pub struct SDeviceMemoryBlock {
    pub mem: vk::DeviceMemory,
    pub size: vk::DeviceSize,
    pub usage_type: EMemoryBlockUsage,

    device: Arc<LogicalDevice>,
}

impl SDeviceMemoryBlock {
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
}

impl Drop for SDeviceMemoryBlock {
    fn drop(&mut self) {
        self.device.memory_allocator.lock().free_device_memory(
            self.mem,
            self.size,
            self.usage_type,
        );
    }
}
