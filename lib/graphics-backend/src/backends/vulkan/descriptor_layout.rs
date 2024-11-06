use std::sync::Arc;

use ash::vk;
use hiarc::Hiarc;

use super::{logical_device::LogicalDevice, sampler::Sampler};

#[derive(Debug, Hiarc)]
pub struct DescriptorSetLayout {
    #[hiarc_skip_unsafe]
    pub layout: vk::DescriptorSetLayout,

    // sampler must not be destroyed before this layout
    _immutable_sampler: Option<Arc<Sampler>>,

    device: Arc<LogicalDevice>,
}

impl DescriptorSetLayout {
    pub fn new(
        device: Arc<LogicalDevice>,
        create_info: &vk::DescriptorSetLayoutCreateInfo,
    ) -> anyhow::Result<Arc<Self>> {
        let layout = unsafe {
            device
                .device
                .create_descriptor_set_layout(create_info, None)
        }?;

        Ok(Arc::new(Self {
            layout,
            device,
            _immutable_sampler: None,
        }))
    }

    pub fn new_with_immutable_sampler(
        device: Arc<LogicalDevice>,
        create_info: &vk::DescriptorSetLayoutCreateInfo,
        immutable_sampler: Arc<Sampler>,
    ) -> anyhow::Result<Arc<Self>> {
        let layout = unsafe {
            device
                .device
                .create_descriptor_set_layout(create_info, None)
        }?;

        Ok(Arc::new(Self {
            layout,
            device,
            _immutable_sampler: Some(immutable_sampler),
        }))
    }
}

impl Drop for DescriptorSetLayout {
    fn drop(&mut self) {
        unsafe {
            self.device
                .device
                .destroy_descriptor_set_layout(self.layout, None);
        }
    }
}
