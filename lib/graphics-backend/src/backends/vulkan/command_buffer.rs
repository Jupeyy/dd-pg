use std::{rc::Rc, sync::Arc};

use ash::vk;

use super::{command_pool::CommandPool, logical_device::LogicalDevice};

#[derive(Debug)]
pub struct CommandBuffers {
    pub command_buffers: Vec<vk::CommandBuffer>,
    pub level: vk::CommandBufferLevel,

    pool: Rc<CommandPool>,
    device: Arc<LogicalDevice>,
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
            device: pool.device.clone(),
            pool,
        }))
    }
}

impl Drop for CommandBuffers {
    fn drop(&mut self) {
        self.device.memory_allocator.lock().free_command_buffers(
            self.pool.command_pool,
            std::mem::take(&mut self.command_buffers),
        );
    }
}
