use std::sync::{Arc, RwLock};

use ash::vk;
use atomic_enum::atomic_enum;
use hiarc::HiArc;
use hiarc_macro::Hiarc;

use super::{
    frame_resources::FrameResources, logical_device::LogicalDevice,
    memory_block::DeviceMemoryBlock, vulkan_mem::ImageAllocationError,
};

#[atomic_enum]
#[derive(PartialEq)]
pub enum ImageLayout {
    Undefined,
    General,
    ShaderRead,
    ColorAttachment,
    DepthStencilAttachment,
    TransferSrc,
    TransferDst,
    Present,
}

impl From<vk::ImageLayout> for ImageLayout {
    fn from(value: vk::ImageLayout) -> Self {
        match value {
            vk::ImageLayout::UNDEFINED => Self::Undefined,
            vk::ImageLayout::GENERAL => Self::General,
            vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL => Self::ColorAttachment,
            vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL => Self::DepthStencilAttachment,
            vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL => Self::ShaderRead,
            vk::ImageLayout::TRANSFER_SRC_OPTIMAL => Self::TransferSrc,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL => Self::TransferDst,
            vk::ImageLayout::PRESENT_SRC_KHR => Self::Present,
            _ => panic!("not yet implemented"),
        }
    }
}

impl Into<vk::ImageLayout> for ImageLayout {
    fn into(self) -> vk::ImageLayout {
        match self {
            Self::Undefined => vk::ImageLayout::UNDEFINED,
            Self::General => vk::ImageLayout::GENERAL,
            Self::ColorAttachment => vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
            Self::DepthStencilAttachment => vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
            Self::ShaderRead => vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            Self::TransferSrc => vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
            Self::TransferDst => vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            Self::Present => vk::ImageLayout::PRESENT_SRC_KHR,
        }
    }
}

#[derive(Debug, Hiarc)]
pub struct Image {
    image: vk::Image,
    bound_memory: RwLock<Option<HiArc<DeviceMemoryBlock>>>,

    pub layout: AtomicImageLayout,

    device: HiArc<LogicalDevice>,

    externally_handled_image: bool,
}

impl Image {
    pub fn new(
        device: HiArc<LogicalDevice>,
        create_info: vk::ImageCreateInfo,
    ) -> anyhow::Result<HiArc<Self>, ImageAllocationError> {
        let image = unsafe { device.device.create_image(&create_info, None) }?;
        Ok(HiArc::new(Self {
            image,
            bound_memory: Default::default(),

            layout: AtomicImageLayout::new(ImageLayout::Undefined),
            device,

            externally_handled_image: false,
        }))
    }

    pub fn from_external_resource(device: HiArc<LogicalDevice>, image: vk::Image) -> HiArc<Self> {
        HiArc::new(Self {
            image,
            bound_memory: Default::default(),
            device,
            layout: AtomicImageLayout::new(ImageLayout::Undefined),
            externally_handled_image: true,
        })
    }

    pub fn bind(
        &self,
        mem: HiArc<DeviceMemoryBlock>,
        offset: vk::DeviceSize,
    ) -> anyhow::Result<(), ImageAllocationError> {
        match unsafe {
            self.device.device.bind_image_memory(
                self.image,
                mem.inner_arc().mem(&mut FrameResources::new(None)),
                offset,
            )
        } {
            Ok(res) => {
                *self.bound_memory.write().unwrap() = Some(mem);
                Ok(res)
            }
            Err(_) => Err(ImageAllocationError::BindMemoryToImageFailed),
        }
    }

    pub fn get_image_memory_requirements(&self) -> vk::MemoryRequirements {
        unsafe { self.device.device.get_image_memory_requirements(self.image) }
    }

    pub fn img(self: &Arc<Self>, frame_resources: &mut FrameResources) -> vk::Image {
        frame_resources.images.push(self.clone());

        self.image
    }
}

impl Drop for Image {
    fn drop(&mut self) {
        if !self.externally_handled_image {
            unsafe { self.device.device.destroy_image(self.image, None) };
        }
    }
}
