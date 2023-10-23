use std::sync::{Arc, RwLock};

use ash::vk;

use super::{
    image::Image, logical_device::LogicalDevice, memory_block::SDeviceMemoryBlock,
    vulkan_mem::ImageAllocationError,
};

#[derive(Debug)]
pub struct ImageView {
    pub image_view: vk::ImageView,
    pub bound_memory: RwLock<Option<Arc<SDeviceMemoryBlock>>>,

    _image: Arc<Image>,
    device: Arc<LogicalDevice>,
}

impl ImageView {
    pub fn new(
        device: Arc<LogicalDevice>,
        image: Arc<Image>,
        create_info: vk::ImageViewCreateInfo,
    ) -> anyhow::Result<Arc<Self>, ImageAllocationError> {
        let image_view = unsafe { device.device.create_image_view(&create_info, None) }?;
        Ok(Arc::new(Self {
            image_view,
            bound_memory: Default::default(),

            _image: image,
            device,
        }))
    }
}

impl Drop for ImageView {
    fn drop(&mut self) {
        self.device
            .memory_allocator
            .lock()
            .free_image_view(self.image_view);
    }
}
