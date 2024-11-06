use std::sync::Arc;

use anyhow::anyhow;
use ash::vk;
use hiarc::Hiarc;

use super::{
    frame_resources::FrameResources, image_view::ImageView, logical_device::LogicalDevice,
    render_pass::RenderPass,
};

#[derive(Debug, Hiarc)]
pub struct Framebuffer {
    #[hiarc_skip_unsafe]
    pub buffer: vk::Framebuffer,

    pub attachments: Vec<Arc<ImageView>>,

    render_pass: Arc<RenderPass>,
    device: Arc<LogicalDevice>,
}

impl Framebuffer {
    pub fn new(
        device: Arc<LogicalDevice>,
        render_pass: Arc<RenderPass>,
        attachments: Vec<Arc<ImageView>>,
        swapchain_extent: vk::Extent2D,
    ) -> anyhow::Result<Self> {
        let attachment_infos: Vec<vk::ImageView> = attachments
            .iter()
            .map(|a| a.view(&mut FrameResources::new(None)))
            .collect();

        let framebuffer_info = vk::FramebufferCreateInfo::default()
            .render_pass(render_pass.pass.pass)
            .attachments(&attachment_infos)
            .width(swapchain_extent.width)
            .height(swapchain_extent.height)
            .layers(1);

        let buffer = unsafe { device.device.create_framebuffer(&framebuffer_info, None) }
            .map_err(|err| anyhow!("Creating the framebuffers failed: {err}"))?;

        Ok(Self {
            buffer,
            device,
            render_pass,
            attachments,
        })
    }

    pub fn transition_images(&self) -> anyhow::Result<()> {
        for (index, attachment) in self.attachments.iter().enumerate() {
            attachment.image.layout.store(
                self.render_pass.attachment_infos[index],
                std::sync::atomic::Ordering::SeqCst,
            );
        }
        Ok(())
    }
}

impl Drop for Framebuffer {
    fn drop(&mut self) {
        unsafe {
            self.device.device.destroy_framebuffer(self.buffer, None);
        }
    }
}
