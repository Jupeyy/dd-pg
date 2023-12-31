use std::rc::Rc;

use ash::vk;
use hiarc::HiRc;
use hiarc_macro::Hiarc;

use super::{command_pool::CommandPool, frame_resources::RenderThreadFrameResources};

#[derive(Debug, Hiarc)]
pub struct CommandBuffers {
    command_buffers: Vec<vk::CommandBuffer>,
    pub level: vk::CommandBufferLevel,

    pool: HiRc<CommandPool>,
}

impl CommandBuffers {
    pub fn new(
        pool: HiRc<CommandPool>,
        level: vk::CommandBufferLevel,
        count: usize,
    ) -> anyhow::Result<HiRc<Self>> {
        let mut alloc_info = vk::CommandBufferAllocateInfo::default();
        alloc_info.command_pool = pool.command_pool;
        alloc_info.level = level;
        alloc_info.command_buffer_count = count as u32;

        let command_buffers = unsafe { pool.device.device.allocate_command_buffers(&alloc_info) }?;

        Ok(HiRc::new(CommandBuffers {
            command_buffers,
            level,
            pool,
        }))
    }

    pub fn from_pool(
        command_buffers: Vec<vk::CommandBuffer>,
        level: vk::CommandBufferLevel,

        pool: HiRc<CommandPool>,
    ) -> HiRc<Self> {
        HiRc::new(Self {
            command_buffers,
            level,
            pool,
        })
    }

    pub fn get(
        self: &Rc<Self>,
        frame_resources: &mut RenderThreadFrameResources,
    ) -> vk::CommandBuffer {
        frame_resources.command_buffers.push(self.clone());

        self.command_buffers[0]
    }
}

impl Drop for CommandBuffers {
    fn drop(&mut self) {
        if self.level == vk::CommandBufferLevel::PRIMARY {
            self.pool
                .primary_command_buffers_in_pool
                .borrow_mut()
                .append(&mut self.command_buffers);
        } else {
            self.pool
                .secondary_command_buffers_in_pool
                .borrow_mut()
                .append(&mut self.command_buffers);
        }
    }
}
