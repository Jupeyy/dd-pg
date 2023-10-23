use std::sync::Arc;

use ash::vk;

use super::logical_device::LogicalDevice;

#[derive(Debug)]
pub struct DescriptorSetLayout {
    pub layout: vk::DescriptorSetLayout,

    device: Arc<LogicalDevice>,
}

impl DescriptorSetLayout {
    pub fn new(
        device: Arc<LogicalDevice>,
        create_info: vk::DescriptorSetLayoutCreateInfo,
    ) -> anyhow::Result<Arc<Self>> {
        let layout = unsafe {
            device
                .device
                .create_descriptor_set_layout(&create_info, None)
        }?;

        Ok(Arc::new(Self { layout, device }))
    }
}

impl Drop for DescriptorSetLayout {
    fn drop(&mut self) {
        self.device
            .memory_allocator
            .lock()
            .free_descriptor_set_layout(self.layout);
    }
}
