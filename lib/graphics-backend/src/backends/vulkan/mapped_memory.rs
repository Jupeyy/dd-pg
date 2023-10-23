use std::sync::Arc;

use ash::vk;

use super::{logical_device::LogicalDevice, memory_block::SDeviceMemoryBlock};

#[derive(Debug)]
pub struct MappedMemory {
    mapped_mem: *mut u8,

    mem: Arc<SDeviceMemoryBlock>,

    device: Arc<LogicalDevice>,
}

unsafe impl Send for MappedMemory {}
unsafe impl Sync for MappedMemory {}

impl MappedMemory {
    pub fn new(
        device: Arc<LogicalDevice>,
        mem: Arc<SDeviceMemoryBlock>,
        offset: vk::DeviceSize,
    ) -> anyhow::Result<Arc<Self>> {
        let mapped_mem = unsafe {
            device
                .device
                .map_memory(mem.mem, offset, vk::WHOLE_SIZE, vk::MemoryMapFlags::empty())
        }? as *mut u8;
        Ok(Arc::new(Self {
            mapped_mem,
            device,
            mem,
        }))
    }

    pub unsafe fn get_mem(&self) -> *mut u8 {
        self.mapped_mem
    }
}

impl Drop for MappedMemory {
    fn drop(&mut self) {
        self.device
            .memory_allocator
            .lock()
            .unmap_device_memory(self.mem.mem);
    }
}
