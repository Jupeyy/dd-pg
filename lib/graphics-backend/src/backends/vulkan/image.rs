use std::sync::{Arc, RwLock};

use ash::vk;

use super::{
    logical_device::LogicalDevice, memory_block::SDeviceMemoryBlock,
    vulkan_mem::ImageAllocationError,
};

#[derive(Debug)]
pub struct Image {
    pub image: vk::Image,
    pub bound_memory: RwLock<Option<Arc<SDeviceMemoryBlock>>>,

    device: Arc<LogicalDevice>,
}

impl Image {
    pub fn new(
        device: Arc<LogicalDevice>,
        create_info: vk::ImageCreateInfo,
    ) -> anyhow::Result<Arc<Self>, ImageAllocationError> {
        let image = unsafe { device.device.create_image(&create_info, None) }?;
        Ok(Arc::new(Self {
            image,
            bound_memory: Default::default(),
            device,
        }))
    }

    pub fn bind(
        &self,
        mem: Arc<SDeviceMemoryBlock>,
        offset: vk::DeviceSize,
    ) -> anyhow::Result<(), ImageAllocationError> {
        match unsafe {
            self.device
                .device
                .bind_image_memory(self.image, mem.mem, offset)
        } {
            Ok(res) => {
                *self.bound_memory.write().unwrap() = Some(mem);
                Ok(res)
            }
            Err(_) => Err(ImageAllocationError::BindMemoryToImageFailed),
        }
    }
}

impl Drop for Image {
    fn drop(&mut self) {
        self.device.memory_allocator.lock().free_image(self.image);
    }
}

pub trait GetImg {
    fn img(&self) -> vk::Image;
}

impl GetImg for Image {
    fn img(&self) -> vk::Image {
        self.image
    }
}

pub struct ImageFakeForSwapchainImgs {
    pub img: vk::Image,
}

impl GetImg for ImageFakeForSwapchainImgs {
    fn img(&self) -> vk::Image {
        self.img
    }
}
