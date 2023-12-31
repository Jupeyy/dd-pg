use std::sync::Arc;

use ash::vk;
use graphics_backend_traits::plugin::SamplerAddressMode;
use graphics_types::commands::StreamDataMax;
use hiarc::HiArc;
use hiarc_macro::Hiarc;
use num_derive::FromPrimitive;

use super::{
    buffer::Buffer,
    compiler::compiler::ShaderCompiler,
    descriptor_pool::DescriptorPool,
    descriptor_set::{DescriptorSet, DescriptorSets},
    frame::FrameCanvasIndex,
    image::Image,
    image_view::ImageView,
    logical_device::LogicalDevice,
    mapped_memory::MappedMemory,
    memory::{MemoryBlock, MemoryImageBlock},
    memory_block::DeviceMemoryBlock,
    pipeline_cache::PipelineCacheInner,
    pipeline_manager::PipelineCreationAttributes,
    render_fill_manager::RenderCommandExecuteBuffer,
    render_group::{COLOR_MASK_TYPE_COUNT, STENCIL_OP_TYPE_COUNT},
    render_pass::CanvasSetup,
    vulkan_allocator::VulkanAllocator,
};

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum EMemoryBlockUsage {
    Texture = 0,
    Buffer,
    Stream,
    Staging,

    // whenever dummy is used, make sure to deallocate all memory
    Dummy,
}

/************************
 * STRUCT DEFINITIONS
 ************************/
#[derive(Debug, Clone, Copy)]
pub enum DescriptorPoolType {
    CombineImgageAndSampler,
    Image,
    Sampler,
    Uniform,
}

#[derive(Debug, Clone, Hiarc)]
pub struct DeviceDescriptorPools {
    pub pools: Vec<HiArc<DescriptorPool>>,
    pub default_alloc_size: vk::DeviceSize,
    pub pool_ty: DescriptorPoolType,
}

impl DeviceDescriptorPools {
    pub fn new(
        device: &HiArc<LogicalDevice>,
        default_alloc_size: vk::DeviceSize,
        pool_ty: DescriptorPoolType,
    ) -> anyhow::Result<HiArc<parking_lot::Mutex<Self>>> {
        let mut pool = DeviceDescriptorPools {
            pools: Default::default(),
            default_alloc_size,
            pool_ty,
        };
        VulkanAllocator::allocate_descriptor_pool(
            device,
            &mut pool,
            StreamDataMax::MaxTextures as usize,
        )?;
        Ok(HiArc::new(parking_lot::Mutex::new(pool)))
    }
}

#[derive(Debug)]
pub enum TextureData {
    Tex2D {
        img: HiArc<Image>,
        img_mem: MemoryImageBlock,
        img_view: HiArc<ImageView>,

        vk_standard_textured_descr_set: HiArc<DescriptorSets>,
    },
    Tex3D {
        img_3d: HiArc<Image>,
        img_3d_mem: MemoryImageBlock,
        img_3d_view: HiArc<ImageView>,

        vk_standard_3d_textured_descr_set: HiArc<DescriptorSets>,
    },
}

impl TextureData {
    pub fn unwrap_3d_descr(&self) -> &HiArc<DescriptorSets> {
        match self {
            TextureData::Tex2D { .. } => panic!("not a 3d texture"),
            TextureData::Tex3D {
                vk_standard_3d_textured_descr_set,
                ..
            } => vk_standard_3d_textured_descr_set,
        }
    }

    pub fn unwrap_2d_descr(&self) -> &HiArc<DescriptorSets> {
        match self {
            TextureData::Tex2D {
                vk_standard_textured_descr_set,
                ..
            } => &vk_standard_textured_descr_set,
            TextureData::Tex3D { .. } => panic!("not a 2d texture"),
        }
    }

    pub fn unwrap_img_2d(&self) -> &HiArc<Image> {
        match self {
            TextureData::Tex2D { img, .. } => img,
            TextureData::Tex3D { .. } => panic!("not a 2d texture"),
        }
    }
}

#[derive(Debug)]
pub struct CTexture {
    pub data: TextureData,

    pub width: usize,
    pub height: usize,
    pub depth: usize,

    pub mip_map_count: u32,
}

#[derive(Debug)]
pub struct BufferObjectMem {
    pub mem: HiArc<MemoryBlock>,
}

#[derive(Debug)]
pub struct BufferObject {
    pub buffer_object: BufferObjectMem,

    pub cur_buffer: HiArc<Buffer>,
    pub cur_buffer_offset: usize,
}

#[derive(Debug)]
pub struct StreamedUniformBuffer {
    pub uniform_sets: [HiArc<DescriptorSet>; 2],
}

#[derive(Debug)]
pub struct ShaderModule {
    pub vert_shader_module: vk::ShaderModule,
    pub frag_shader_module: vk::ShaderModule,

    vk_device: HiArc<LogicalDevice>,
}

impl ShaderModule {
    pub fn new(
        vert_shader_module: vk::ShaderModule,
        frag_shader_module: vk::ShaderModule,
        vk_device: &HiArc<LogicalDevice>,
    ) -> Self {
        Self {
            vert_shader_module,
            frag_shader_module,
            vk_device: vk_device.clone(),
        }
    }
}

impl Drop for ShaderModule {
    fn drop(&mut self) {
        if self.vert_shader_module != vk::ShaderModule::null() {
            unsafe {
                self.vk_device
                    .device
                    .destroy_shader_module(self.vert_shader_module, None);
            }
        }

        if self.frag_shader_module != vk::ShaderModule::null() {
            unsafe {
                self.vk_device
                    .device
                    .destroy_shader_module(self.frag_shader_module, None);
            }
        }
    }
}

#[derive(FromPrimitive, Copy, Clone)]
#[repr(u32)]
pub enum EVulkanBackendAddressModes {
    Repeat = 0,
    ClampEdges = 1,

    Count = 2,
}

#[derive(Debug, FromPrimitive, Copy, Clone, PartialEq)]
#[repr(u32)]
pub enum EVulkanBackendBlendModes {
    Alpha = 0,
    None = 1,
    Additive = 2,
}
pub const BLEND_MODE_COUNT: usize = 3;

#[derive(Debug, FromPrimitive, Copy, Clone, PartialEq)]
#[repr(u32)]
pub enum EVulkanBackendClipModes {
    None = 0,
    DynamicScissorAndViewport = 1,

    Count = 2,
}

const MAX_TEXTURE_MODES: usize = 2;

#[derive(Debug, Default, Copy, Clone, FromPrimitive, PartialEq)]
pub enum RenderPassType {
    #[default]
    Single = 0,
    // switch around 2 framebuffers to use each other as
    // input attachments
    Switching1,
    Switching2,
}

#[derive(Debug, Clone)]
pub struct PipelineCreationAttributesEx {
    pub address_mode_index: usize,
    pub is_textured: bool,
}

#[derive(Debug, Clone)]
pub struct PipelineCreationProps {
    pub attr: PipelineCreationAttributes,
    pub attr_ex: PipelineCreationAttributesEx,
}

#[derive(Debug, Clone)]
pub struct PipelineCreationOneByOne {
    pub device: HiArc<LogicalDevice>,
    pub shader_compiler: Arc<ShaderCompiler>,
    pub swapchain_extent: vk::Extent2D,
    pub render_pass: vk::RenderPass,

    pub pipeline_cache: Option<HiArc<PipelineCacheInner>>,
}

#[derive(Debug, Default)]
pub enum PipelineContainerItem {
    Normal {
        pipeline_layout: vk::PipelineLayout,
        pipeline: vk::Pipeline,
    },
    MaybeUninit {
        pipeline_and_layout: parking_lot::Mutex<Option<(vk::PipelineLayout, vk::Pipeline)>>,

        creation_props: PipelineCreationProps,
        creation_data: PipelineCreationOneByOne,
    },
    #[default]
    None,
}

#[derive(Debug, Clone)]
pub enum PipelineContainerCreateMode {
    AtOnce,
    OneByOne(PipelineCreationOneByOne),
}

#[derive(Debug)]
pub struct PipelineContainer {
    // 3 blend modes - 2 viewport & scissor modes - 2 texture modes - 4 stencil modes - 3 color mask types - 3 sampler modes
    pub pipelines: [[[[[[PipelineContainerItem; SAMPLER_TYPES_COUNT]; COLOR_MASK_TYPE_COUNT];
        STENCIL_OP_TYPE_COUNT]; MAX_TEXTURE_MODES];
        EVulkanBackendClipModes::Count as usize]; BLEND_MODE_COUNT],

    pub(crate) mode: PipelineContainerCreateMode,

    device: HiArc<LogicalDevice>,
}

impl PipelineContainer {
    pub fn new(device: HiArc<LogicalDevice>, mode: PipelineContainerCreateMode) -> Self {
        Self {
            pipelines: Default::default(),
            mode,
            device,
        }
    }
}

impl Drop for PipelineContainer {
    fn drop(&mut self) {
        for pipes_list_list_list_list in &mut self.pipelines {
            for pipes_list_list_list in pipes_list_list_list_list {
                for pipes_list_list in pipes_list_list_list {
                    for pipes_list in pipes_list_list {
                        for pipes in pipes_list {
                            for pipe in pipes {
                                fn destroy_pipe_and_layout(
                                    device: &HiArc<LogicalDevice>,
                                    pipeline_layout: &mut vk::PipelineLayout,
                                    pipeline: &mut vk::Pipeline,
                                ) {
                                    if *pipeline_layout != vk::PipelineLayout::null() {
                                        unsafe {
                                            device
                                                .device
                                                .destroy_pipeline_layout(*pipeline_layout, None);
                                        }
                                    }
                                    *pipeline_layout = vk::PipelineLayout::null();
                                    if *pipeline != vk::Pipeline::null() {
                                        unsafe {
                                            device.device.destroy_pipeline(*pipeline, None);
                                        }
                                    }
                                    *pipeline = vk::Pipeline::null();
                                }
                                match pipe {
                                    PipelineContainerItem::Normal {
                                        pipeline_layout,
                                        pipeline,
                                    } => {
                                        destroy_pipe_and_layout(
                                            &self.device,
                                            pipeline_layout,
                                            pipeline,
                                        );
                                    }
                                    PipelineContainerItem::MaybeUninit {
                                        pipeline_and_layout,
                                        ..
                                    } => {
                                        let mut pipe_and_layout = pipeline_and_layout.lock();
                                        if let Some((pipeline_layout, pipeline)) =
                                            &mut *pipe_and_layout
                                        {
                                            destroy_pipe_and_layout(
                                                &self.device,
                                                pipeline_layout,
                                                pipeline,
                                            );
                                        }
                                        *pipe_and_layout = None;
                                    }
                                    PipelineContainerItem::None => {
                                        // nothing to do, some pipelines are not intialized (e.g. if the pipeline always expects texturing)
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[derive(Debug, FromPrimitive, Copy, Clone, PartialEq)]
#[repr(u32)]
pub enum ESupportedSamplerTypes {
    Repeat = 0,
    ClampToEdge,
    Texture2DArray,
}
pub const SAMPLER_TYPES_COUNT: usize = 3;

impl Into<SamplerAddressMode> for ESupportedSamplerTypes {
    fn into(self) -> SamplerAddressMode {
        match self {
            ESupportedSamplerTypes::Repeat => SamplerAddressMode::Repeat,
            ESupportedSamplerTypes::ClampToEdge => SamplerAddressMode::ClampToEdge,
            ESupportedSamplerTypes::Texture2DArray => SamplerAddressMode::Texture2DArray,
        }
    }
}

#[derive(Debug)]
pub struct SwapChainImageBase {
    pub image: HiArc<Image>,
    pub img_mem: MemoryImageBlock,
    pub img_view: HiArc<ImageView>,
}

#[derive(Debug)]
pub struct SwapChainImageFull {
    pub base: SwapChainImageBase,

    pub texture_descr_sets: HiArc<DescriptorSets>,
}

#[derive(Debug, Clone)]
pub struct VKDelayedBufferCleanupItem {
    pub buffer: Option<HiArc<Buffer>>,
    pub mem: HiArc<DeviceMemoryBlock>,
    pub mapped_data: Option<(isize, HiArc<MappedMemory>)>,
}

#[derive(Debug, Default)]
pub struct ThreadCommandGroup {
    pub render_pass: RenderPassType,

    pub render_pass_index: usize,
    pub canvas_index: FrameCanvasIndex,

    pub cur_frame_index: u32,

    pub in_order_id: usize,

    pub cmds: Vec<RenderCommandExecuteBuffer>,
}

#[derive(Debug)]
pub enum RenderThreadEvent {
    ClearFrame(u32),
    ClearFrames,
}

#[derive(Debug)]
pub struct RenderThreadInner {
    pub thread: Option<std::thread::JoinHandle<()>>,
    pub finished: bool,
    pub started: bool,

    pub events: Vec<RenderThreadEvent>,
    pub render_calls: Vec<(ThreadCommandGroup, HiArc<CanvasSetup>)>,
}

#[derive(Debug)]
pub struct RenderThread {
    pub inner: parking_lot::Mutex<RenderThreadInner>,
    pub cond: parking_lot::Condvar,
}
