use ash::vk;
use hiarc::HiArc;
use hiarc_macro::Hiarc;

use super::{logical_device::LogicalDevice, sampler::Sampler};

#[derive(Debug, Hiarc)]
pub struct DescriptorSetLayout {
    pub layout: vk::DescriptorSetLayout,

    // sampler must not be destroyed before this layout
    _immutable_sampler: Option<HiArc<Sampler>>,

    device: HiArc<LogicalDevice>,
}

impl DescriptorSetLayout {
    pub fn new(
        device: HiArc<LogicalDevice>,
        create_info: vk::DescriptorSetLayoutCreateInfo,
    ) -> anyhow::Result<HiArc<Self>> {
        let layout = unsafe {
            device
                .device
                .create_descriptor_set_layout(&create_info, None)
        }?;

        Ok(HiArc::new(Self {
            layout,
            device,
            _immutable_sampler: None,
        }))
    }

    pub fn new_with_immutable_sampler(
        device: HiArc<LogicalDevice>,
        create_info: vk::DescriptorSetLayoutCreateInfo,
        immutable_sampler: HiArc<Sampler>,
    ) -> anyhow::Result<HiArc<Self>> {
        let layout = unsafe {
            device
                .device
                .create_descriptor_set_layout(&create_info, None)
        }?;

        Ok(HiArc::new(Self {
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
