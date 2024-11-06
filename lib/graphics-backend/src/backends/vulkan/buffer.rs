use std::sync::{Arc, RwLock};

use ash::vk;
use hiarc::Hiarc;

use super::{
    frame_resources::FrameResources, logical_device::LogicalDevice, memory_block::DeviceMemoryBlock,
};

#[derive(Debug, Hiarc)]
pub struct Buffer {
    #[hiarc_skip_unsafe]
    buffer: vk::Buffer,
    bound_memory: RwLock<Option<Arc<DeviceMemoryBlock>>>,

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

    pub fn bind(&self, mem: Arc<DeviceMemoryBlock>) -> anyhow::Result<()> {
        unsafe {
            self.device.device.bind_buffer_memory(
                self.buffer,
                mem.mem(&mut FrameResources::new(None)),
                0,
            )
        }?;
        *self.bound_memory.write().unwrap() = Some(mem);
        Ok(())
    }

    pub fn get_buffer_memory_requirements(&self) -> vk::MemoryRequirements {
        unsafe {
            self.device
                .device
                .get_buffer_memory_requirements(self.buffer)
        }
    }

    pub fn get_buffer(self: &Arc<Self>, frame_resources: &mut FrameResources) -> vk::Buffer {
        frame_resources.buffers.push(self.clone());

        self.buffer
    }
}

impl Drop for Buffer {
    fn drop(&mut self) {
        unsafe { self.device.device.destroy_buffer(self.buffer, None) };
    }
}
