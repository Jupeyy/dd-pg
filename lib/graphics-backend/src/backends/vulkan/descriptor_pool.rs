use std::sync::{atomic::AtomicU64, Arc};

use ash::vk;

use super::logical_device::LogicalDevice;

#[derive(Debug)]
pub struct DescriptorPool {
    pub pool: spin::Mutex<vk::DescriptorPool>,
    pub size: vk::DeviceSize,
    pub cur_size: Arc<AtomicU64>,

    pub device: Arc<LogicalDevice>,
}

impl DescriptorPool {
    pub fn new(
        device: Arc<LogicalDevice>,
        create_info: vk::DescriptorPoolCreateInfo,
    ) -> anyhow::Result<Arc<Self>> {
        assert!(
            create_info.pool_size_count == 1,
            "for simplicty reasons, only one pool type is allowed per descriptor pool"
        );
        let pool =
            spin::Mutex::new(unsafe { device.device.create_descriptor_pool(&create_info, None) }?);

        Ok(Arc::new(Self {
            pool,
            size: create_info.pool_size_count as vk::DeviceSize,
            cur_size: Default::default(),
            device,
        }))
    }
}

impl Drop for DescriptorPool {
    fn drop(&mut self) {
        self.device
            .memory_allocator
            .lock()
            .free_descriptor_pool(*self.pool.lock());
    }
}
