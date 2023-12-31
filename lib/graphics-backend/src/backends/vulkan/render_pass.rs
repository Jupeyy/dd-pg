use std::sync::Arc;

use anyhow::anyhow;
use ash::vk::{self};
use graphics_backend_traits::plugin::BackendCustomPipeline;
use hiarc::HiArc;
use hiarc_macro::Hiarc;

use super::compiler::compiler::ShaderCompiler;
use super::image::ImageLayout;
use super::pipeline_cache::PipelineCacheInner;
use super::render_setup::{CanvasSetupNative, CanvasSetupSwitching, RenderSetupCreationType};
use super::sub_render_pass::SubRenderPass;
use super::vulkan_allocator::VulkanAllocator;
use super::vulkan_device::DescriptorLayouts;
use super::vulkan_types::{DeviceDescriptorPools, RenderPassType};
use super::{logical_device::LogicalDevice, phy_device::PhyDevice, vulkan_device::Device};

#[derive(Debug, Hiarc)]
pub struct RenderPassInner {
    pub pass: vk::RenderPass,
    device: HiArc<LogicalDevice>,
}

impl RenderPassInner {
    pub fn new(
        device: HiArc<LogicalDevice>,
        create_render_pass_info: vk::RenderPassCreateInfo,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            pass: unsafe {
                device
                    .device
                    .create_render_pass(&create_render_pass_info, None)
            }?,
            device,
        })
    }
}

impl Drop for RenderPassInner {
    fn drop(&mut self) {
        unsafe {
            self.device.device.destroy_render_pass(self.pass, None);
        }
    }
}

#[derive(Debug, Hiarc)]
pub struct RenderPass {
    pub pass: RenderPassInner,

    pub subpasses: Vec<SubRenderPass>,

    pub attachment_infos: Vec<ImageLayout>,
}

impl RenderPass {
    pub fn new(
        device: &HiArc<LogicalDevice>,
        layouts: &DescriptorLayouts,
        custom_pipes: &Arc<parking_lot::RwLock<Vec<Box<dyn BackendCustomPipeline>>>>,
        pipeline_cache: &Option<HiArc<PipelineCacheInner>>,
        runtime_threadpool: &Arc<rayon::ThreadPool>,
        shader_compiler: &Arc<ShaderCompiler>,
        swapchain_extent: vk::Extent2D,
        dependencies: &[vk::SubpassDependency],
        subpasses: &[vk::SubpassDescription],
        attachments: &[vk::AttachmentDescription],
        compile_one_by_one: bool,
    ) -> anyhow::Result<HiArc<Self>> {
        let mut create_render_pass_info = vk::RenderPassCreateInfo::default();
        create_render_pass_info.attachment_count = attachments.len() as u32;
        create_render_pass_info.p_attachments = attachments.as_ptr();
        create_render_pass_info.subpass_count = subpasses.len() as u32;
        create_render_pass_info.p_subpasses = subpasses.as_ptr();
        create_render_pass_info.dependency_count = dependencies.len() as u32;
        create_render_pass_info.p_dependencies = dependencies.as_ptr();

        let pass = RenderPassInner::new(device.clone(), create_render_pass_info)?;

        let mut subpasses = Vec::new();
        subpasses.push(SubRenderPass::new(
            device,
            layouts,
            custom_pipes,
            pipeline_cache,
            runtime_threadpool,
            shader_compiler,
            swapchain_extent,
            pass.pass,
            compile_one_by_one,
        )?);

        Ok(HiArc::new(Self {
            pass,
            subpasses,

            attachment_infos: attachments.iter().map(|a| a.final_layout.into()).collect(),
        }))
    }
}

/// all rendering related stuff
/// such as renderpasses and framebuffers
#[derive(Debug, Hiarc)]
pub struct CanvasSetup {
    pub surf_format: vk::SurfaceFormatKHR,
    pub stencil_format: vk::Format,

    pub switching: CanvasSetupSwitching,
    pub native: CanvasSetupNative,
}

impl CanvasSetup {
    pub fn new(
        device: &HiArc<LogicalDevice>,
        layouts: &DescriptorLayouts,
        custom_pipes: &Arc<parking_lot::RwLock<Vec<Box<dyn BackendCustomPipeline>>>>,
        pipeline_cache: &Option<HiArc<PipelineCacheInner>>,
        standard_texture_descr_pool: &HiArc<parking_lot::Mutex<DeviceDescriptorPools>>,
        mem_allocator: &HiArc<parking_lot::Mutex<VulkanAllocator>>,
        runtime_threadpool: &Arc<rayon::ThreadPool>,
        shader_compiler: &Arc<ShaderCompiler>,
        creation_type: RenderSetupCreationType,
        compile_one_by_one: bool,
    ) -> anyhow::Result<HiArc<Self>> {
        let surf_format = match &creation_type {
            RenderSetupCreationType::Swapchain((swapchain, _)) => swapchain.format,
            RenderSetupCreationType::Offscreen { img_format, .. } => *img_format,
        };

        let native = CanvasSetupNative::new(
            device,
            layouts,
            custom_pipes,
            pipeline_cache,
            mem_allocator,
            runtime_threadpool,
            shader_compiler,
            creation_type,
            compile_one_by_one,
        )?;

        let (switching, stencil_format) = CanvasSetupSwitching::new(
            device,
            layouts,
            custom_pipes,
            pipeline_cache,
            standard_texture_descr_pool,
            mem_allocator,
            runtime_threadpool,
            shader_compiler,
            native.swap_chain_image_view_list.len(),
            surf_format.format,
            native.swap_img_and_viewport_extent,
            compile_one_by_one,
        )?;

        Ok(HiArc::new(Self {
            surf_format,

            switching,
            native,

            stencil_format,
        }))
    }

    pub fn sub_render_pass(&self, ty: RenderPassType) -> &SubRenderPass {
        match ty {
            RenderPassType::Single => &self.native.render_pass.subpasses[0],
            RenderPassType::Switching1 => &self.switching.passes[0].render_pass.subpasses[0],
            RenderPassType::Switching2 => &self.switching.passes[1].render_pass.subpasses[0],
        }
    }

    pub fn swap_chain_image_count(&self) -> usize {
        self.native.swap_chain_images.len()
    }

    fn multisampling_description(
        phy_device: &HiArc<PhyDevice>,
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
        device: &HiArc<LogicalDevice>,
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
        stencil_format: vk::Format,
        phy_device: &HiArc<PhyDevice>,
        has_multi_sampling: bool,
        multi_sampling_count: u32,
    ) -> vk::AttachmentDescription {
        let mut stencil_attachment = vk::AttachmentDescription::default();
        stencil_attachment.format = stencil_format;
        stencil_attachment.samples = if !has_multi_sampling {
            vk::SampleCountFlags::TYPE_1
        } else {
            Device::get_sample_count(multi_sampling_count, &phy_device.limits)
        };
        stencil_attachment.load_op = vk::AttachmentLoadOp::DONT_CARE;
        stencil_attachment.store_op = vk::AttachmentStoreOp::DONT_CARE;
        stencil_attachment.stencil_load_op = vk::AttachmentLoadOp::LOAD;
        stencil_attachment.stencil_store_op = vk::AttachmentStoreOp::STORE;
        stencil_attachment.initial_layout = vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL;
        stencil_attachment.final_layout = vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL;
        stencil_attachment
    }

    pub fn create_render_pass_switching(
        stencil_format: vk::Format,

        logical_device: &HiArc<LogicalDevice>,
        layouts: &DescriptorLayouts,
        custom_pipes: &Arc<parking_lot::RwLock<Vec<Box<dyn BackendCustomPipeline>>>>,
        pipeline_cache: &Option<HiArc<PipelineCacheInner>>,
        runtime_threadpool: &Arc<rayon::ThreadPool>,
        shader_compiler: &Arc<ShaderCompiler>,
        swapchain_extent: vk::Extent2D,

        has_multi_sampling: bool,
        format: vk::Format,
        compile_one_by_one: bool,
    ) -> anyhow::Result<HiArc<RenderPass>> {
        let has_multi_sampling_targets = has_multi_sampling;

        let phy_device = &logical_device.phy_device;

        let multi_sampling_color_attachment = Self::multisampling_description(phy_device, format);
        let mut color_attachment =
            Self::attachment_description(logical_device, format, has_multi_sampling);
        color_attachment.final_layout = vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL;

        let stencil_attachment = Self::stencil_description_pass(
            stencil_format,
            phy_device,
            has_multi_sampling,
            phy_device.config.read().unwrap().multi_sampling_count,
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

        RenderPass::new(
            logical_device,
            layouts,
            custom_pipes,
            pipeline_cache,
            runtime_threadpool,
            shader_compiler,
            swapchain_extent,
            &dependencies,
            &subpasses,
            &attachments[0..attachment_count],
            compile_one_by_one,
        )
        .map_err(|err| {
            anyhow!(format!(
                "Creating the render pass for switching pass failed: {}",
                err
            ))
        })
    }

    pub fn create_render_pass_impl(
        logical_device: &HiArc<LogicalDevice>,
        layouts: &DescriptorLayouts,
        custom_pipes: &Arc<parking_lot::RwLock<Vec<Box<dyn BackendCustomPipeline>>>>,
        pipeline_cache: &Option<HiArc<PipelineCacheInner>>,
        runtime_threadpool: &Arc<rayon::ThreadPool>,
        shader_compiler: &Arc<ShaderCompiler>,
        swapchain_extent: vk::Extent2D,
        has_multi_sampling: bool,
        format: vk::Format,
        compile_one_by_one: bool,
    ) -> anyhow::Result<HiArc<RenderPass>> {
        let has_multi_sampling_targets = has_multi_sampling;

        let phy_device = &logical_device.phy_device;

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

        RenderPass::new(
            logical_device,
            layouts,
            custom_pipes,
            pipeline_cache,
            runtime_threadpool,
            shader_compiler,
            swapchain_extent,
            &dependencies,
            &subpasses,
            &attachments[0..attachment_count],
            compile_one_by_one,
        )
        .map_err(|err| anyhow!(format!("Creating the render pass failed: {}", err)))
    }
}
