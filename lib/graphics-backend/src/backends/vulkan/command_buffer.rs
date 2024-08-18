use std::rc::Rc;

use ash::vk;
use hiarc::Hiarc;

use super::{command_pool::CommandPool, frame_resources::RenderThreadFrameResources};

#[derive(Debug, Hiarc)]
pub struct CommandBuffers {
    #[hiarc_skip_unsafe]
    command_buffers: Vec<vk::CommandBuffer>,
    #[hiarc_skip_unsafe]
    pub level: vk::CommandBufferLevel,

    pool: Rc<CommandPool>,
}

impl CommandBuffers {
    pub fn new(
        pool: Rc<CommandPool>,
        level: vk::CommandBufferLevel,
        count: usize,
    ) -> anyhow::Result<Rc<Self>> {
        let mut alloc_info = vk::CommandBufferAllocateInfo::default();
        alloc_info.command_pool = pool.command_pool;
        alloc_info.level = level;
        alloc_info.command_buffer_count = count as u32;

        let command_buffers = unsafe { pool.device.device.allocate_command_buffers(&alloc_info) }?;

        Ok(Rc::new(CommandBuffers {
            command_buffers,
            level,
            pool,
        }))
    }

    pub fn from_pool(
        command_buffers: Vec<vk::CommandBuffer>,
        level: vk::CommandBufferLevel,

        pool: Rc<CommandPool>,
    ) -> Rc<Self> {
        Rc::new(Self {
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
