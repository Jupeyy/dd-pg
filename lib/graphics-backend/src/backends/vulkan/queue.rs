use std::sync::Arc;

use ash::vk;

#[derive(Debug)]
pub struct Queue {
    pub graphics_queue: vk::Queue,
    pub present_queue: vk::Queue,
}

impl Queue {
    pub fn new(graphics_queue: vk::Queue, present_queue: vk::Queue) -> Arc<spin::Mutex<Self>> {
        Arc::new(spin::Mutex::new(Self {
            graphics_queue,
            present_queue,
        }))
    }
}
