use std::sync::{Arc, RwLock};

use ash::vk;

use super::{logical_device::LogicalDevice, memory_block::SDeviceMemoryBlock};

#[derive(Debug)]
pub struct Buffer {
    pub buffer: vk::Buffer,
    pub bound_memory: RwLock<Option<Arc<SDeviceMemoryBlock>>>,

    device: Arc<LogicalDevice>,
}

impl Buffer {
    pub fn new(
        device: Arc<LogicalDevice>,
        create_info: vk::BufferCreateInfo,
    ) -> anyhow::Result<Arc<Self>> {
        let buffer = unsafe { device.device.create_buffer(&create_info, None) }?;
        Ok(Arc::new(Self {
            buffer,
            bound_memory: Default::default(),
            device,
        }))
    }

    pub fn bind(&self, mem: Arc<SDeviceMemoryBlock>) -> anyhow::Result<()> {
        unsafe {
            self.device
                .device
                .bind_buffer_memory(self.buffer, mem.mem, 0)
        }?;
        *self.bound_memory.write().unwrap() = Some(mem);
        Ok(())
    }
}

impl Drop for Buffer {
    fn drop(&mut self) {
        self.device.memory_allocator.lock().free_buffer(self.buffer);
    }
}
