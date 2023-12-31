use std::sync::atomic::AtomicU64;

use ash::vk;
use hiarc::HiArc;
use hiarc_macro::Hiarc;

use super::logical_device::LogicalDevice;

#[derive(Debug, Hiarc)]
pub struct DescriptorPool {
    pub pool: parking_lot::Mutex<vk::DescriptorPool>,
    pub size: vk::DeviceSize,
    pub cur_size: AtomicU64,

    pub device: HiArc<LogicalDevice>,
}

impl DescriptorPool {
    pub fn new(
        device: HiArc<LogicalDevice>,
        create_info: vk::DescriptorPoolCreateInfo,
    ) -> anyhow::Result<HiArc<Self>> {
        assert!(
            create_info.pool_size_count == 1,
            "for simplicty reasons, only one pool type is allowed per descriptor pool"
        );
        let pool = parking_lot::Mutex::new(unsafe {
            device.device.create_descriptor_pool(&create_info, None)
        }?);

        Ok(HiArc::new(Self {
            pool,
            size: create_info.max_sets as vk::DeviceSize,
            cur_size: Default::default(),
            device,
        }))
    }
}

impl Drop for DescriptorPool {
    fn drop(&mut self) {
        unsafe {
            self.device
                .device
                .destroy_descriptor_pool(*self.pool.lock(), None);
        }
    }
}
