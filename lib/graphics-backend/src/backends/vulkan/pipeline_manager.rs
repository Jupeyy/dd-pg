use std::sync::Arc;

use anyhow::anyhow;
use ash::vk;
use hiarc::HiArc;

use super::{
    compiler::compiler::ShaderCompiler,
    logical_device::LogicalDevice,
    pipeline_cache::PipelineCacheInner,
    render_group::{ColorMaskType, StencilOpType},
    vulkan_device::Device,
    vulkan_types::{EVulkanBackendBlendModes, EVulkanBackendClipModes, ShaderModule},
};

const SHADER_MAIN_FUNC_NAME: [u8; 5] = [b'm', b'a', b'i', b'n', b'\0'];

#[derive(Debug, Clone)]
pub struct PipelineCreationAttributes {
    pub vert_name: String,
    pub frag_name: String,
    pub stride: u32,
    pub input_attributes: Vec<vk::VertexInputAttributeDescription>,
    pub set_layouts: Vec<vk::DescriptorSetLayout>,
    pub push_constants: Vec<vk::PushConstantRange>,
    pub blend_mode: EVulkanBackendBlendModes,
    pub dynamic_mode: EVulkanBackendClipModes,
    pub is_line_prim: bool,

    pub stencil_mode: StencilOpType,
    pub color_mask: ColorMaskType,
}

pub struct PipelineManager<'a> {
    device: &'a HiArc<LogicalDevice>,
    shader_compiler: &'a Arc<ShaderCompiler>,
    swapchain_extent: vk::Extent2D,
    render_pass: vk::RenderPass,

    pipeline_cache: &'a Option<HiArc<PipelineCacheInner>>,
}

impl<'a> PipelineManager<'a> {
    pub fn new(
        device: &'a HiArc<LogicalDevice>,
        shader_compiler: &'a Arc<ShaderCompiler>,
        swapchain_extent: vk::Extent2D,
        render_pass: vk::RenderPass,
        pipeline_cache: &'a Option<HiArc<PipelineCacheInner>>,
    ) -> Self {
        Self {
            device,
            shader_compiler,
            swapchain_extent,
            render_pass,
            pipeline_cache,
        }
    }

    pub fn load_shader(
        shader_compiler: &Arc<ShaderCompiler>,
        file_name: &str,
    ) -> anyhow::Result<Vec<u32>> {
        let f = shader_compiler
            .shader_files
            .get(file_name)
            .ok_or(anyhow!("Shader file was not loaded: {file_name}"))?;

        Ok(f.clone())
    }

    pub fn create_shader_module(
        device: &HiArc<LogicalDevice>,
        code: &Vec<u32>,
    ) -> anyhow::Result<vk::ShaderModule> {
        let mut create_info = vk::ShaderModuleCreateInfo::default();
        create_info.code_size = code.len() * std::mem::size_of::<u32>();
        create_info.p_code = code.as_ptr() as _;

        unsafe { device.device.create_shader_module(&create_info, None) }
            .map_err(|err| anyhow!("Shader module was not created: {err}"))
    }

    fn create_shaders(
        device: &HiArc<LogicalDevice>,
        shader_compiler: &Arc<ShaderCompiler>,
        vert_name: &str,
        frag_name: &str,
        shader_stages: &mut [vk::PipelineShaderStageCreateInfo; 2],
    ) -> anyhow::Result<ShaderModule> {
        let vert_data_buff = Self::load_shader(shader_compiler, vert_name)?;
        let frag_data_buff = Self::load_shader(shader_compiler, frag_name)?;

        let vert_shader_module = Self::create_shader_module(device, &vert_data_buff)?;
        let frag_shader_module = Self::create_shader_module(device, &frag_data_buff)?;

        let vert_shader_stage_info = &mut shader_stages[0];
        *vert_shader_stage_info = vk::PipelineShaderStageCreateInfo::default();
        vert_shader_stage_info.stage = vk::ShaderStageFlags::VERTEX;
        vert_shader_stage_info.module = vert_shader_module;
        vert_shader_stage_info.p_name = SHADER_MAIN_FUNC_NAME.as_ptr() as *const i8;

        let frag_shader_stage_info = &mut shader_stages[1];
        *frag_shader_stage_info = vk::PipelineShaderStageCreateInfo::default();
        frag_shader_stage_info.stage = vk::ShaderStageFlags::FRAGMENT;
        frag_shader_stage_info.module = frag_shader_module;
        frag_shader_stage_info.p_name = SHADER_MAIN_FUNC_NAME.as_ptr() as *const i8;

        Ok(ShaderModule::new(
            vert_shader_module,
            frag_shader_module,
            device,
        ))
    }

    fn get_standard_pipeline_info(
        device: &HiArc<LogicalDevice>,
        swapchain_extent: vk::Extent2D,
        input_assembly: &mut vk::PipelineInputAssemblyStateCreateInfo,
        viewport: &mut vk::Viewport,
        scissor: &mut vk::Rect2D,
        viewport_state: &mut vk::PipelineViewportStateCreateInfo,
        rasterizer: &mut vk::PipelineRasterizationStateCreateInfo,
        multisampling: &mut vk::PipelineMultisampleStateCreateInfo,
        color_blend_attachment: &mut vk::PipelineColorBlendAttachmentState,
        color_blending: &mut vk::PipelineColorBlendStateCreateInfo,
        blend_mode: EVulkanBackendBlendModes,
        color_mask: ColorMaskType,
    ) -> bool {
        input_assembly.topology = vk::PrimitiveTopology::TRIANGLE_LIST;
        input_assembly.primitive_restart_enable = vk::FALSE;

        viewport.x = 0.0;
        viewport.y = 0.0;
        viewport.width = swapchain_extent.width as f32;
        viewport.height = swapchain_extent.height as f32;
        viewport.min_depth = 0.0;
        viewport.max_depth = 1.0;

        scissor.offset = vk::Offset2D { x: 0, y: 0 };
        scissor.extent = swapchain_extent;

        viewport_state.viewport_count = 1;
        viewport_state.p_viewports = viewport;
        viewport_state.scissor_count = 1;
        viewport_state.p_scissors = scissor;

        rasterizer.depth_clamp_enable = vk::FALSE;
        rasterizer.rasterizer_discard_enable = vk::FALSE;
        rasterizer.polygon_mode = vk::PolygonMode::FILL;
        rasterizer.line_width = 1.0;
        rasterizer.cull_mode = vk::CullModeFlags::NONE;
        rasterizer.front_face = vk::FrontFace::CLOCKWISE;
        rasterizer.depth_bias_enable = vk::FALSE;

        multisampling.sample_shading_enable = vk::FALSE;
        multisampling.rasterization_samples = Device::get_sample_count(
            device
                .phy_device
                .config
                .read()
                .unwrap()
                .multi_sampling_count,
            &device.phy_device.limits,
        );

        color_blend_attachment.color_write_mask = match color_mask {
            ColorMaskType::WriteAll => {
                vk::ColorComponentFlags::R
                    | vk::ColorComponentFlags::G
                    | vk::ColorComponentFlags::B
                    | vk::ColorComponentFlags::A
            }
            ColorMaskType::WriteColorOnly => {
                vk::ColorComponentFlags::R | vk::ColorComponentFlags::G | vk::ColorComponentFlags::B
            }
            ColorMaskType::WriteAlphaOnly => vk::ColorComponentFlags::A,
            ColorMaskType::WriteNone => vk::ColorComponentFlags::empty(),
        };

        color_blend_attachment.blend_enable = if blend_mode == EVulkanBackendBlendModes::None {
            vk::FALSE
        } else {
            vk::TRUE
        };

        let src_blend_factor_color = match blend_mode {
            EVulkanBackendBlendModes::Additive => vk::BlendFactor::ONE,
            EVulkanBackendBlendModes::Alpha => vk::BlendFactor::SRC_ALPHA,
            EVulkanBackendBlendModes::None => vk::BlendFactor::SRC_COLOR,
        };

        let dst_blend_factor_color = match blend_mode {
            EVulkanBackendBlendModes::Additive => vk::BlendFactor::ONE_MINUS_SRC_ALPHA,
            EVulkanBackendBlendModes::Alpha => vk::BlendFactor::ONE_MINUS_SRC_ALPHA,
            EVulkanBackendBlendModes::None => vk::BlendFactor::SRC_COLOR,
        };

        let src_blend_factor_alpha = match blend_mode {
            EVulkanBackendBlendModes::Additive => vk::BlendFactor::ONE,
            EVulkanBackendBlendModes::Alpha => vk::BlendFactor::SRC_ALPHA,
            EVulkanBackendBlendModes::None => vk::BlendFactor::SRC_COLOR,
        };

        let dst_blend_factor_alpha = match blend_mode {
            EVulkanBackendBlendModes::Additive => vk::BlendFactor::ZERO,
            EVulkanBackendBlendModes::Alpha => vk::BlendFactor::ONE_MINUS_SRC_ALPHA,
            EVulkanBackendBlendModes::None => vk::BlendFactor::SRC_COLOR,
        };

        color_blend_attachment.src_color_blend_factor = src_blend_factor_color;
        color_blend_attachment.dst_color_blend_factor = dst_blend_factor_color;
        color_blend_attachment.color_blend_op = vk::BlendOp::ADD;
        color_blend_attachment.src_alpha_blend_factor = src_blend_factor_alpha;
        color_blend_attachment.dst_alpha_blend_factor = dst_blend_factor_alpha;
        color_blend_attachment.alpha_blend_op = vk::BlendOp::ADD;

        color_blending.logic_op_enable = vk::FALSE;
        color_blending.logic_op = vk::LogicOp::COPY;
        color_blending.attachment_count = 1;
        color_blending.p_attachments = color_blend_attachment;
        color_blending.blend_constants[0] = 0.0;
        color_blending.blend_constants[1] = 0.0;
        color_blending.blend_constants[2] = 0.0;
        color_blending.blend_constants[3] = 0.0;

        true
    }

    pub fn create_graphics_pipeline_ex(
        &self,
        attrs: &[PipelineCreationAttributes],
    ) -> anyhow::Result<Vec<(vk::Pipeline, vk::PipelineLayout)>> {
        #[derive(Debug, Default)]
        struct CreateStack {
            shader_stages: [vk::PipelineShaderStageCreateInfo; 2],

            vertex_input_info: vk::PipelineVertexInputStateCreateInfo,
            binding_description: vk::VertexInputBindingDescription,

            input_assembly: vk::PipelineInputAssemblyStateCreateInfo,
            viewport: vk::Viewport,
            scissor: vk::Rect2D,
            viewport_state: vk::PipelineViewportStateCreateInfo,
            rasterizer: vk::PipelineRasterizationStateCreateInfo,
            multisampling: vk::PipelineMultisampleStateCreateInfo,
            color_blend_attachment: vk::PipelineColorBlendAttachmentState,
            color_blending: vk::PipelineColorBlendStateCreateInfo,
            stencil_state: vk::PipelineDepthStencilStateCreateInfo,

            pipeline_layout_info: vk::PipelineLayoutCreateInfo,

            push_constants: Vec<vk::PushConstantRange>,

            pipeline_info: vk::GraphicsPipelineCreateInfo,

            dynamic_state_create: vk::PipelineDynamicStateCreateInfo,

            dynamic_states: [vk::DynamicState; 2],

            module: Option<ShaderModule>,
        }

        let mut create_stack: Vec<CreateStack> = Vec::with_capacity(attrs.len());
        create_stack.resize_with(attrs.len(), || Default::default());

        let mut res: Vec<(vk::Pipeline, vk::PipelineLayout)> = Default::default();

        for (index, attr) in attrs.iter().enumerate() {
            let create_stack = &mut create_stack[index];
            create_stack.module = Some(Self::create_shaders(
                self.device,
                self.shader_compiler,
                &attr.vert_name,
                &attr.frag_name,
                &mut create_stack.shader_stages,
            )?);

            create_stack.binding_description.binding = 0;
            create_stack.binding_description.stride = attr.stride;
            create_stack.binding_description.input_rate = vk::VertexInputRate::VERTEX;

            create_stack
                .vertex_input_info
                .vertex_binding_description_count = 1;
            create_stack
                .vertex_input_info
                .vertex_attribute_description_count = attr.input_attributes.len() as u32;
            create_stack.vertex_input_info.p_vertex_binding_descriptions =
                &create_stack.binding_description;
            create_stack
                .vertex_input_info
                .p_vertex_attribute_descriptions = attr.input_attributes.as_ptr();

            create_stack.dynamic_states = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];

            Self::get_standard_pipeline_info(
                self.device,
                self.swapchain_extent,
                &mut create_stack.input_assembly,
                &mut create_stack.viewport,
                &mut create_stack.scissor,
                &mut create_stack.viewport_state,
                &mut create_stack.rasterizer,
                &mut create_stack.multisampling,
                &mut create_stack.color_blend_attachment,
                &mut create_stack.color_blending,
                attr.blend_mode,
                attr.color_mask,
            );
            create_stack.input_assembly.topology = if attr.is_line_prim {
                vk::PrimitiveTopology::LINE_LIST
            } else {
                vk::PrimitiveTopology::TRIANGLE_LIST
            };

            create_stack.pipeline_layout_info.set_layout_count = attr.set_layouts.len() as u32;
            create_stack.pipeline_layout_info.p_set_layouts = if !attr.set_layouts.is_empty() {
                attr.set_layouts.as_ptr()
            } else {
                std::ptr::null()
            };

            // TODO: workaround since WGSL has no offset attribute
            let push_constants_reranged: Vec<vk::PushConstantRange> = attr
                .push_constants
                .iter()
                .map(|p| {
                    let mut p = *p;
                    p.size += p.offset;
                    p.offset = 0;
                    p.stage_flags = vk::ShaderStageFlags::FRAGMENT | vk::ShaderStageFlags::VERTEX;
                    p
                })
                .collect();
            for p in push_constants_reranged {
                let range_p = p.offset..p.offset + p.size;
                if let Some((range_index, _)) =
                    create_stack
                        .push_constants
                        .iter()
                        .enumerate()
                        .find(|(_, p2)| {
                            let range_p2 = p2.offset..p2.offset + p2.size;

                            range_p.start <= range_p2.end && range_p2.start <= range_p.end
                        })
                {
                    let p2 = &mut create_stack.push_constants[range_index];
                    p2.offset = p2.offset.min(p.offset);
                    p2.size = (p2.offset + p2.size).max(p.offset + p.size) - p2.offset;
                } else {
                    create_stack.push_constants.push(p);
                }
            }
            // workaround end

            create_stack.pipeline_layout_info.push_constant_range_count =
                create_stack.push_constants.len() as u32;
            create_stack.pipeline_layout_info.p_push_constant_ranges =
                if !create_stack.push_constants.is_empty() {
                    create_stack.push_constants.as_ptr()
                } else {
                    std::ptr::null()
                };

            let pipe_layout = unsafe {
                self.device
                    .device
                    .create_pipeline_layout(&create_stack.pipeline_layout_info, None)
            }
            .map_err(|err| anyhow!("Creating pipeline layout failed: {err}"))?;

            create_stack.pipeline_info.stage_count = create_stack.shader_stages.len() as u32;
            create_stack.pipeline_info.p_stages = create_stack.shader_stages.as_ptr();
            create_stack.pipeline_info.p_vertex_input_state = &create_stack.vertex_input_info;
            create_stack.pipeline_info.p_input_assembly_state = &create_stack.input_assembly;
            create_stack.pipeline_info.p_viewport_state = &create_stack.viewport_state;
            create_stack.pipeline_info.p_rasterization_state = &create_stack.rasterizer;
            create_stack.pipeline_info.p_multisample_state = &create_stack.multisampling;
            create_stack.pipeline_info.p_color_blend_state = &create_stack.color_blending;
            match attr.stencil_mode {
                StencilOpType::AlwaysPass => {
                    create_stack.stencil_state.stencil_test_enable = vk::TRUE;
                    create_stack.stencil_state.front.compare_op = vk::CompareOp::ALWAYS;
                    create_stack.stencil_state.front.fail_op = vk::StencilOp::REPLACE;
                    create_stack.stencil_state.front.pass_op = vk::StencilOp::REPLACE;
                    create_stack.stencil_state.front.depth_fail_op = vk::StencilOp::REPLACE;
                    create_stack.stencil_state.front.compare_mask = 0xFF;
                    create_stack.stencil_state.front.write_mask = 0xFF;
                    create_stack.stencil_state.front.reference = 0x1;
                    create_stack.stencil_state.back = create_stack.stencil_state.front;
                    create_stack.pipeline_info.p_depth_stencil_state = &create_stack.stencil_state;
                }
                StencilOpType::OnlyWhenPassed => {
                    create_stack.stencil_state.stencil_test_enable = vk::TRUE;
                    create_stack.stencil_state.front.compare_op = vk::CompareOp::EQUAL;
                    create_stack.stencil_state.front.fail_op = vk::StencilOp::KEEP;
                    create_stack.stencil_state.front.pass_op = vk::StencilOp::KEEP;
                    create_stack.stencil_state.front.depth_fail_op = vk::StencilOp::KEEP;
                    create_stack.stencil_state.front.compare_mask = 0xFF;
                    create_stack.stencil_state.front.write_mask = 0xFF;
                    create_stack.stencil_state.front.reference = 0x1;
                    create_stack.stencil_state.back = create_stack.stencil_state.front;
                    create_stack.pipeline_info.p_depth_stencil_state = &create_stack.stencil_state;
                }
                StencilOpType::OnlyWhenNotPassed => {
                    create_stack.stencil_state.stencil_test_enable = vk::TRUE;
                    create_stack.stencil_state.front.compare_op = vk::CompareOp::NOT_EQUAL;
                    create_stack.stencil_state.front.fail_op = vk::StencilOp::KEEP;
                    create_stack.stencil_state.front.pass_op = vk::StencilOp::KEEP;
                    create_stack.stencil_state.front.depth_fail_op = vk::StencilOp::KEEP;
                    create_stack.stencil_state.front.compare_mask = 0xFF;
                    create_stack.stencil_state.front.write_mask = 0xFF;
                    create_stack.stencil_state.front.reference = 0x1;
                    create_stack.stencil_state.back = create_stack.stencil_state.front;
                    create_stack.pipeline_info.p_depth_stencil_state = &create_stack.stencil_state;
                }
                StencilOpType::None => {
                    // nothing to do
                    create_stack.stencil_state.stencil_test_enable = vk::FALSE;
                    create_stack.pipeline_info.p_depth_stencil_state = &create_stack.stencil_state;
                }
            }
            create_stack.pipeline_info.layout = pipe_layout;
            create_stack.pipeline_info.render_pass = self.render_pass;
            create_stack.pipeline_info.subpass = 0;
            create_stack.pipeline_info.base_pipeline_handle = vk::Pipeline::null();

            create_stack.dynamic_state_create.dynamic_state_count =
                create_stack.dynamic_states.len() as u32;
            create_stack.dynamic_state_create.p_dynamic_states =
                create_stack.dynamic_states.as_ptr();

            if attr.dynamic_mode == EVulkanBackendClipModes::DynamicScissorAndViewport {
                create_stack.pipeline_info.p_dynamic_state = &create_stack.dynamic_state_create;
            }
        }

        let pipeline_infos: Vec<vk::GraphicsPipelineCreateInfo> = create_stack
            .iter()
            .map(|create_stack| create_stack.pipeline_info)
            .collect();
        let pipelines_res = unsafe {
            self.device.device.create_graphics_pipelines(
                self.pipeline_cache
                    .as_ref()
                    .map(|cache| cache.cache)
                    .unwrap_or(vk::PipelineCache::null()),
                &pipeline_infos,
                None,
            )
        };
        let pipelines = match pipelines_res {
            Ok(pipelines) => pipelines,
            Err((pipelines, res)) => match res {
                vk::Result::PIPELINE_COMPILE_REQUIRED_EXT => pipelines,
                _ => {
                    return Err(anyhow!("Creating the graphic pipeline failed: {res}"));
                }
            },
        };

        for (index, pipeline) in pipelines.into_iter().enumerate() {
            res.push((pipeline, create_stack[index].pipeline_info.layout));
        }

        Ok(res)
    }
}
