use anyhow::anyhow;
use ash::vk;
use hiarc::HiArc;

use super::{
    frame_resources::FrameResources, image_view::ImageView, logical_device::LogicalDevice,
    render_pass::RenderPass,
};

#[derive(Debug)]
pub struct Framebuffer {
    pub buffer: vk::Framebuffer,

    pub attachments: Vec<HiArc<ImageView>>,

    render_pass: HiArc<RenderPass>,
    device: HiArc<LogicalDevice>,
}

impl Framebuffer {
    pub fn new(
        device: HiArc<LogicalDevice>,
        render_pass: HiArc<RenderPass>,
        attachments: Vec<HiArc<ImageView>>,
        swapchain_extent: vk::Extent2D,
    ) -> anyhow::Result<Self> {
        let attachment_infos: Vec<vk::ImageView> = attachments
            .iter()
            .map(|a| a.inner_arc().view(&mut FrameResources::new(None)))
            .collect();

        let mut framebuffer_info = vk::FramebufferCreateInfo::default();
        framebuffer_info.render_pass = render_pass.pass.pass;
        framebuffer_info.attachment_count = attachment_infos.len() as u32;
        framebuffer_info.p_attachments = attachment_infos.as_ptr();
        framebuffer_info.width = swapchain_extent.width;
        framebuffer_info.height = swapchain_extent.height;
        framebuffer_info.layers = 1;

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
                self.render_pass.attachment_infos[index].into(),
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
