use std::sync::Arc;

use anyhow::anyhow;
use ash::vk;

use super::vulkan_types::{
    PipelineContainer, RenderPassType, SwapChainImageBase, SwapChainImageFull,
};
use super::{logical_device::LogicalDevice, phy_device::PhyDevice, vulkan_device::Device};

#[derive(Debug, Default)]
pub struct SubRenderPass {
    pub standard_pipeline: PipelineContainer,
    pub standard_line_pipeline: PipelineContainer,
    pub standard_stencil_only_pipeline: PipelineContainer,
    pub standard_stencil_when_passed_pipeline: PipelineContainer,
    pub standard_stencil_pipeline: PipelineContainer,
    pub standard_3d_pipeline: PipelineContainer,
    pub blur_pipeline: PipelineContainer,
    pub tile_pipeline: PipelineContainer,
    pub tile_border_pipeline: PipelineContainer,
    pub tile_border_line_pipeline: PipelineContainer,
    pub prim_ex_pipeline: PipelineContainer,
    pub prim_ex_rotationless_pipeline: PipelineContainer,
    pub sprite_multi_pipeline: PipelineContainer,
    pub sprite_multi_push_pipeline: PipelineContainer,
    pub quad_pipeline: PipelineContainer,
    pub quad_push_pipeline: PipelineContainer,
}

#[derive(Debug)]
pub struct RenderPass {
    pub pass: vk::RenderPass,

    pub subpasses: Vec<SubRenderPass>,
}

impl Default for RenderPass {
    fn default() -> Self {
        let mut subpasses = Vec::new();
        subpasses.resize_with(1, || Default::default());
        Self {
            pass: Default::default(),
            subpasses,
        }
    }
}

/// offscreen in a sense that it is never visible on the screen
/// not like in double buffering
#[derive(Debug, Default)]
pub struct OffscreenSurface {
    pub image_list: Vec<SwapChainImageFull>,
    pub multi_sampling_images: Vec<SwapChainImageBase>,
}

#[derive(Debug, Default)]
pub struct RenderSetupSwitchingPass {
    pub surface: OffscreenSurface,
    // render into a offscreen framebuffer first
    pub render_pass: RenderPass,
    pub framebuffer_list: Vec<vk::Framebuffer>,
}

#[derive(Debug, Default)]
pub struct RenderSetupSwitching {
    pub stencil_list_for_pass_transition: Vec<SwapChainImageBase>,
    // switching images generally only write to offscreen buffers
    pub passes: [RenderSetupSwitchingPass; 2],
}

#[derive(Debug, Default)]
pub struct RenderSetupNative {
    // swap chain images, that are created by the surface
    pub swap_chain_images: Vec<vk::Image>,
    pub swap_chain_image_view_list: Vec<vk::ImageView>,
    pub swap_chain_multi_sampling_images: Vec<SwapChainImageBase>,

    pub render_pass: RenderPass,
    pub framebuffer_list: Vec<vk::Framebuffer>,
}

/// all rendering related stuff
/// such as renderpasses and framebuffers
#[derive(Debug)]
pub struct RenderSetup {
    pub vk_surf_format: vk::SurfaceFormatKHR,
    pub stencil_format: vk::Format,

    pub switching: RenderSetupSwitching,
    pub native: RenderSetupNative,
}

impl RenderSetup {
    pub fn new() -> Self {
        Self {
            vk_surf_format: Default::default(),

            switching: Default::default(),
            native: Default::default(),

            stencil_format: Default::default(),
        }
    }

    pub fn sub_render_pass(&self, ty: RenderPassType) -> &SubRenderPass {
        match ty {
            RenderPassType::Single => &self.native.render_pass.subpasses[0],
            RenderPassType::Switching1 => &self.switching.passes[0].render_pass.subpasses[0],
            RenderPassType::Switching2 => &self.switching.passes[1].render_pass.subpasses[0],
        }
    }

    pub fn sub_render_pass_mut(&mut self, ty: RenderPassType) -> &mut SubRenderPass {
        match ty {
            RenderPassType::Single => &mut self.native.render_pass.subpasses[0],
            RenderPassType::Switching1 => &mut self.switching.passes[0].render_pass.subpasses[0],
            RenderPassType::Switching2 => &mut self.switching.passes[1].render_pass.subpasses[0],
        }
    }

    fn multisampling_description(
        phy_device: &Arc<PhyDevice>,
        format: vk::Format,
    ) -> vk::AttachmentDescription {
        let mut multi_sampling_color_attachment = vk::AttachmentDescription::default();
        multi_sampling_color_attachment.format = format;
        multi_sampling_color_attachment.samples = Device::get_sample_count(
            phy_device.config.read().unwrap().multi_sampling_count,
            &phy_device.limits,
        );
        multi_sampling_color_attachment.load_op = vk::AttachmentLoadOp::CLEAR;
        multi_sampling_color_attachment.store_op = vk::AttachmentStoreOp::DONT_CARE;
        multi_sampling_color_attachment.stencil_load_op = vk::AttachmentLoadOp::DONT_CARE;
        multi_sampling_color_attachment.stencil_store_op = vk::AttachmentStoreOp::DONT_CARE;
        multi_sampling_color_attachment.initial_layout = vk::ImageLayout::UNDEFINED;
        multi_sampling_color_attachment.final_layout = vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL;
        multi_sampling_color_attachment
    }

    fn attachment_description(
        device: &Arc<LogicalDevice>,
        format: vk::Format,
        has_multi_sampling: bool,
    ) -> vk::AttachmentDescription {
        let mut color_attachment = vk::AttachmentDescription::default();
        color_attachment.format = format;
        color_attachment.samples = vk::SampleCountFlags::TYPE_1;

        // if this attachment is a result of previous multi sampling => don't clear
        color_attachment.load_op = if !has_multi_sampling {
            vk::AttachmentLoadOp::CLEAR
        } else {
            vk::AttachmentLoadOp::DONT_CARE
        };
        color_attachment.store_op = vk::AttachmentStoreOp::STORE;
        color_attachment.stencil_load_op = vk::AttachmentLoadOp::DONT_CARE;
        color_attachment.stencil_store_op = vk::AttachmentStoreOp::DONT_CARE;
        color_attachment.initial_layout = vk::ImageLayout::UNDEFINED;
        color_attachment.final_layout = device.final_layout();
        color_attachment
    }

    fn stencil_description_pass(
        &self,
        phy_device: &Arc<PhyDevice>,
        has_multi_sampling: bool,
        multi_sampling_count: u32,
        force_load: bool,
    ) -> vk::AttachmentDescription {
        let mut stencil_attachment = vk::AttachmentDescription::default();
        stencil_attachment.format = self.stencil_format;
        stencil_attachment.samples = if !has_multi_sampling {
            vk::SampleCountFlags::TYPE_1
        } else {
            Device::get_sample_count(multi_sampling_count, &phy_device.limits)
        };
        stencil_attachment.load_op = vk::AttachmentLoadOp::DONT_CARE;
        stencil_attachment.store_op = vk::AttachmentStoreOp::DONT_CARE;
        stencil_attachment.stencil_load_op = if force_load {
            vk::AttachmentLoadOp::LOAD
        } else {
            vk::AttachmentLoadOp::CLEAR
        };
        stencil_attachment.stencil_store_op = vk::AttachmentStoreOp::STORE;
        stencil_attachment.initial_layout = if force_load {
            vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL
        } else {
            vk::ImageLayout::UNDEFINED
        };
        stencil_attachment.final_layout = vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL;
        stencil_attachment
    }

    pub fn create_render_pass_switching(
        &mut self,
        phy_device: &Arc<PhyDevice>,
        logical_device: &Arc<LogicalDevice>,
        has_multi_sampling: bool,
        format: vk::Format,
    ) -> anyhow::Result<vk::RenderPass> {
        let has_multi_sampling_targets = has_multi_sampling;

        let multi_sampling_color_attachment = Self::multisampling_description(phy_device, format);
        let mut color_attachment =
            Self::attachment_description(logical_device, format, has_multi_sampling);
        color_attachment.final_layout = vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL;

        let stencil_attachment = self.stencil_description_pass(
            phy_device,
            has_multi_sampling,
            phy_device.config.read().unwrap().multi_sampling_count,
            true,
        );

        let mut color_attachment_ref = vk::AttachmentReference::default();
        color_attachment_ref.layout = vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL;

        let mut multi_sampling_color_attachment_ref = vk::AttachmentReference::default();
        multi_sampling_color_attachment_ref.layout = vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL;

        let mut stencil_buffer_ref = vk::AttachmentReference::default();
        stencil_buffer_ref.layout = vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL;

        /*let mut color_attachment_from_first_pass_as_input_ref = vk::AttachmentReference::default();
        color_attachment_from_first_pass_as_input_ref.layout =
            vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL;*/

        let mut attachments: [vk::AttachmentDescription; 6] = Default::default();
        let mut attachment_count = 0;
        attachments[attachment_count] = color_attachment;
        color_attachment_ref.attachment = attachment_count as u32;
        attachment_count += 1;
        if has_multi_sampling_targets {
            attachments[attachment_count] = multi_sampling_color_attachment;
            multi_sampling_color_attachment_ref.attachment = attachment_count as u32;
            attachment_count += 1;
        }
        attachments[attachment_count] = stencil_attachment;
        stencil_buffer_ref.attachment = attachment_count as u32;
        attachment_count += 1;
        // previous swapping attachment as input attachment
        /*attachments[attachment_count] = input_attachment;
        color_attachment_from_first_pass_as_input_ref.attachment = attachment_count as u32;
        attachment_count += 1;*/

        let mut subpasses = [vk::SubpassDescription::default()];
        subpasses[0].pipeline_bind_point = vk::PipelineBindPoint::GRAPHICS;
        subpasses[0].color_attachment_count = 1;
        subpasses[0].p_color_attachments = if has_multi_sampling_targets {
            &multi_sampling_color_attachment_ref
        } else {
            &color_attachment_ref
        };
        subpasses[0].p_resolve_attachments = if has_multi_sampling_targets {
            &color_attachment_ref
        } else {
            std::ptr::null()
        };

        //subpasses[0].input_attachment_count = 1;
        //subpasses[0].p_input_attachments = &color_attachment_from_first_pass_as_input_ref;
        subpasses[0].p_depth_stencil_attachment = &stencil_buffer_ref;

        let mut dependencies = [
            vk::SubpassDependency::default(),
            //vk::SubpassDependency::default(),
        ];
        dependencies[0].src_subpass = vk::SUBPASS_EXTERNAL;
        dependencies[0].dst_subpass = 0;
        dependencies[0].src_stage_mask = vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT
            | vk::PipelineStageFlags::LATE_FRAGMENT_TESTS;
        dependencies[0].src_access_mask = vk::AccessFlags::COLOR_ATTACHMENT_WRITE
            | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE;
        dependencies[0].dst_stage_mask = vk::PipelineStageFlags::FRAGMENT_SHADER
            | vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT
            | vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS;
        dependencies[0].dst_access_mask = vk::AccessFlags::SHADER_READ
            | vk::AccessFlags::COLOR_ATTACHMENT_WRITE
            | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE;
        dependencies[0].dependency_flags = vk::DependencyFlags::BY_REGION;

        /*dependencies[1].src_subpass = 0;
        dependencies[1].dst_subpass = 0;
        dependencies[1].src_stage_mask = vk::PipelineStageFlags::FRAGMENT_SHADER
            | vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT;
        dependencies[1].src_access_mask = vk::AccessFlags::MEMORY_READ
            | vk::AccessFlags::MEMORY_WRITE
            | vk::AccessFlags::COLOR_ATTACHMENT_WRITE
            | vk::AccessFlags::SHADER_READ;
        dependencies[1].dst_stage_mask = vk::PipelineStageFlags::FRAGMENT_SHADER
            | vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT;
        dependencies[1].dst_access_mask = vk::AccessFlags::MEMORY_READ
            | vk::AccessFlags::MEMORY_WRITE
            | vk::AccessFlags::COLOR_ATTACHMENT_WRITE
            | vk::AccessFlags::SHADER_READ;
        dependencies[1].dependency_flags = vk::DependencyFlags::BY_REGION;*/

        let mut create_render_pass_info = vk::RenderPassCreateInfo::default();
        create_render_pass_info.attachment_count = attachment_count as u32;
        create_render_pass_info.p_attachments = attachments.as_ptr();
        create_render_pass_info.subpass_count = subpasses.len() as u32;
        create_render_pass_info.p_subpasses = subpasses.as_ptr();
        create_render_pass_info.dependency_count = dependencies.len() as u32;
        create_render_pass_info.p_dependencies = dependencies.as_ptr();

        let res = unsafe {
            logical_device
                .device
                .create_render_pass(&create_render_pass_info, None)
        };
        match res {
            Ok(res) => Ok(res),
            Err(err) => Err(anyhow!(format!(
                "Creating the render pass for switching pass failed: {}",
                err
            ))),
        }
    }

    pub fn create_render_pass_impl(
        &mut self,
        phy_device: &Arc<PhyDevice>,
        logical_device: &Arc<LogicalDevice>,
        has_multi_sampling: bool,
        format: vk::Format,
    ) -> anyhow::Result<vk::RenderPass> {
        let has_multi_sampling_targets = has_multi_sampling;
        let multi_sampling_color_attachment = Self::multisampling_description(phy_device, format);

        let color_attachment =
            Self::attachment_description(logical_device, format, has_multi_sampling);

        let mut color_attachment_ref = vk::AttachmentReference::default();
        color_attachment_ref.layout = vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL;

        let mut multi_sampling_color_attachment_ref = vk::AttachmentReference::default();
        multi_sampling_color_attachment_ref.layout = vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL;

        let mut attachments: [vk::AttachmentDescription; 6] = Default::default();
        let mut attachment_count = 0;
        attachments[attachment_count] = color_attachment;
        color_attachment_ref.attachment = attachment_count as u32;
        attachment_count += 1;
        if has_multi_sampling_targets {
            attachments[attachment_count] = multi_sampling_color_attachment;
            multi_sampling_color_attachment_ref.attachment = attachment_count as u32;
            attachment_count += 1;
        }

        let mut subpasses = [vk::SubpassDescription::default()];
        subpasses[0].pipeline_bind_point = vk::PipelineBindPoint::GRAPHICS;
        subpasses[0].color_attachment_count = 1;
        subpasses[0].p_color_attachments = if has_multi_sampling_targets {
            &multi_sampling_color_attachment_ref
        } else {
            &color_attachment_ref
        };
        subpasses[0].p_resolve_attachments = if has_multi_sampling_targets {
            &color_attachment_ref
        } else {
            std::ptr::null()
        };

        let mut dependencies = [vk::SubpassDependency::default()];
        dependencies[0].src_subpass = vk::SUBPASS_EXTERNAL;
        dependencies[0].dst_subpass = 0;
        dependencies[0].src_stage_mask = vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT;
        dependencies[0].src_access_mask = vk::AccessFlags::empty();
        dependencies[0].dst_stage_mask = vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT;
        dependencies[0].dst_access_mask = vk::AccessFlags::COLOR_ATTACHMENT_WRITE;
        dependencies[0].dependency_flags = vk::DependencyFlags::BY_REGION;

        let mut create_render_pass_info = vk::RenderPassCreateInfo::default();
        create_render_pass_info.attachment_count = attachment_count as u32;
        create_render_pass_info.p_attachments = attachments.as_ptr();
        create_render_pass_info.subpass_count = subpasses.len() as u32;
        create_render_pass_info.p_subpasses = subpasses.as_ptr();
        create_render_pass_info.dependency_count = dependencies.len() as u32;
        create_render_pass_info.p_dependencies = dependencies.as_ptr();

        let res = unsafe {
            logical_device
                .device
                .create_render_pass(&create_render_pass_info, None)
        };
        match res {
            Ok(res) => Ok(res),
            Err(err) => Err(anyhow!(format!("Creating the render pass failed: {}", err))),
        }
    }
}

#[derive(Debug)]
pub struct RenderSetupGroup {
    pub onscreen: RenderSetup,
    pub offscreen: RenderSetup,
}

impl RenderSetupGroup {
    pub fn get(&self) -> &RenderSetup {
        &self.onscreen
    }

    pub fn get_mut(&mut self) -> &mut RenderSetup {
        &mut self.onscreen
    }
}
