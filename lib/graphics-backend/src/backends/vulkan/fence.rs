use std::sync::Arc;

use ash::vk;
use hiarc::Hiarc;

use super::logical_device::LogicalDevice;

#[derive(Debug, Hiarc)]
pub struct Fence {
    #[hiarc_skip_unsafe]
    pub fence: vk::Fence,

    device: Arc<LogicalDevice>,
}

impl Fence {
    pub fn new(device: Arc<LogicalDevice>) -> anyhow::Result<Arc<Self>> {
        let mut fence_info = vk::FenceCreateInfo::default();
        fence_info.flags = vk::FenceCreateFlags::SIGNALED;

        let fence = unsafe { device.device.create_fence(&fence_info, None) }?;

        Ok(Arc::new(Self { fence, device }))
    }
}

impl Drop for Fence {
    fn drop(&mut self) {
        unsafe {
            self.device.device.destroy_fence(self.fence, None);
        }
    }
}
