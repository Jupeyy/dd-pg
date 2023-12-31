use ash::vk;
use hiarc::HiArc;
use hiarc_macro::Hiarc;

#[derive(Debug)]
pub struct VkQueues {
    pub graphics_queue: vk::Queue,
    pub present_queue: vk::Queue,
}

#[derive(Debug, Hiarc)]
pub struct Queue {
    pub queues: parking_lot::Mutex<VkQueues>,
}

impl Queue {
    pub fn new(graphics_queue: vk::Queue, present_queue: vk::Queue) -> HiArc<Self> {
        HiArc::new(Self {
            queues: parking_lot::Mutex::new(VkQueues {
                graphics_queue,
                present_queue,
            }),
        })
    }
}
