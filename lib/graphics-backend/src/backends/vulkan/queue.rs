use std::sync::Arc;

use ash::vk;
use hiarc::Hiarc;

#[derive(Debug, Hiarc)]
pub struct VkQueues {
    #[hiarc_skip_unsafe]
    pub graphics_queue: vk::Queue,
    #[hiarc_skip_unsafe]
    pub present_queue: vk::Queue,
}

#[derive(Debug, Hiarc)]
pub struct Queue {
    pub queues: parking_lot::Mutex<VkQueues>,
}

impl Queue {
    pub fn new(graphics_queue: vk::Queue, present_queue: vk::Queue) -> Arc<Self> {
        Arc::new(Self {
            queues: parking_lot::Mutex::new(VkQueues {
                graphics_queue,
                present_queue,
            }),
        })
    }
}
