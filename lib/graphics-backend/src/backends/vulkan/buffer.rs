use std::sync::{Arc, RwLock};

use ash::vk;
use hiarc::HiArc;
use hiarc_macro::Hiarc;

use super::{
    frame_resources::FrameResources, logical_device::LogicalDevice, memory_block::DeviceMemoryBlock,
};

#[derive(Debug, Hiarc)]
pub struct Buffer {
    buffer: vk::Buffer,
    bound_memory: RwLock<Option<HiArc<DeviceMemoryBlock>>>,

    device: HiArc<LogicalDevice>,
}

impl Buffer {
    pub fn new(
        device: HiArc<LogicalDevice>,
        create_info: vk::BufferCreateInfo,
    ) -> anyhow::Result<HiArc<Self>> {
        let buffer = unsafe { device.device.create_buffer(&create_info, None) }?;
        Ok(HiArc::new(Self {
            buffer,
            bound_memory: Default::default(),
            device,
        }))
    }

    pub fn bind(&self, mem: HiArc<DeviceMemoryBlock>) -> anyhow::Result<()> {
        unsafe {
            self.device.device.bind_buffer_memory(
                self.buffer,
                mem.inner_arc().mem(&mut FrameResources::new(None)),
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
