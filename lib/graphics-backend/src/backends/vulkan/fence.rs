use ash::vk;
use hiarc::HiArc;
use hiarc_macro::Hiarc;

use super::logical_device::LogicalDevice;

#[derive(Debug, Hiarc)]
pub struct Fence {
    pub fence: vk::Fence,

    device: HiArc<LogicalDevice>,
}

impl Fence {
    pub fn new(device: HiArc<LogicalDevice>) -> anyhow::Result<HiArc<Self>> {
        let mut fence_info = vk::FenceCreateInfo::default();
        fence_info.flags = vk::FenceCreateFlags::SIGNALED;

        let fence = unsafe { device.device.create_fence(&fence_info, None) }?;

        Ok(HiArc::new(Self { fence, device }))
    }
}

impl Drop for Fence {
    fn drop(&mut self) {
        unsafe {
            self.device.device.destroy_fence(self.fence, None);
        }
    }
}
