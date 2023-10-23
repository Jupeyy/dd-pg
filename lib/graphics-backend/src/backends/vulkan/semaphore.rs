use std::sync::Arc;

use ash::vk;

use super::logical_device::LogicalDevice;

#[derive(Debug)]
pub struct Semaphore {
    pub semaphore: vk::Semaphore,
    pub is_timeline: bool,

    device: Arc<LogicalDevice>,
}

impl Semaphore {
    pub fn new(device: Arc<LogicalDevice>, is_timeline: bool) -> anyhow::Result<Arc<Self>> {
        let extra_info = vk::SemaphoreTypeCreateInfo::builder()
            .semaphore_type(vk::SemaphoreType::TIMELINE_KHR)
            .build();

        let mut create_semaphore_info_builder = vk::SemaphoreCreateInfo::builder();
        if is_timeline {
            create_semaphore_info_builder.p_next = &extra_info as *const _ as *const _;
        }

        let semaphore_info = create_semaphore_info_builder.build();

        let semaphore = unsafe { device.device.create_semaphore(&semaphore_info, None) }?;

        Ok(Arc::new(Self {
            semaphore,
            is_timeline,
            device,
        }))
    }
}

impl Drop for Semaphore {
    fn drop(&mut self) {
        self.device
            .memory_allocator
            .lock()
            .free_semaphore(self.semaphore);
    }
}
