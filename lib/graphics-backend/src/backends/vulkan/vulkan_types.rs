use std::sync::Arc;

use ash::vk;
use graphics_backend_traits::plugin::SamplerAddressMode;
use graphics_types::commands::StreamDataMax;
use hiarc::Hiarc;
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
    memory::{MemoryBlock, MemoryImageBlock},
    pipeline_cache::PipelineCacheInner,
    pipeline_manager::PipelineCreationAttributes,
    pipelines::Pipelines,
    render_fill_manager::RenderCommandExecuteBuffer,
    render_group::{COLOR_MASK_TYPE_COUNT, STENCIL_OP_TYPE_COUNT},
    render_pass::CanvasSetup,
    vulkan_allocator::VulkanAllocator,
};

#[derive(Debug, Hiarc, Copy, Clone, PartialEq)]
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
#[derive(Debug, Hiarc, Clone, Copy)]
pub enum DescriptorPoolType {
    CombineImgageAndSampler,
    Image,
    Sampler,
    Uniform,
}

#[derive(Debug, Clone, Hiarc)]
pub struct DeviceDescriptorPools {
    pub pools: Vec<Arc<DescriptorPool>>,
    pub default_alloc_size: vk::DeviceSize,
    pub pool_ty: DescriptorPoolType,
}

impl DeviceDescriptorPools {
    pub fn new(
        device: &Arc<LogicalDevice>,
        default_alloc_size: vk::DeviceSize,
        pool_ty: DescriptorPoolType,
    ) -> anyhow::Result<Arc<parking_lot::Mutex<Self>>> {
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
        Ok(Arc::new(parking_lot::Mutex::new(pool)))
    }
}

#[derive(Debug, Hiarc)]
pub enum TextureData {
    Tex2D {
        img: Arc<Image>,
        img_mem: MemoryImageBlock,
        img_view: Arc<ImageView>,

        vk_standard_textured_descr_set: Arc<DescriptorSets>,
    },
    Tex3D {
        img_3d: Arc<Image>,
        img_3d_mem: MemoryImageBlock,
        img_3d_view: Arc<ImageView>,

        vk_standard_3d_textured_descr_set: Arc<DescriptorSets>,
    },
}

impl TextureData {
    pub fn unwrap_3d_descr(&self) -> &Arc<DescriptorSets> {
        match self {
            TextureData::Tex2D { .. } => panic!("not a 3d texture"),
            TextureData::Tex3D {
                vk_standard_3d_textured_descr_set,
                ..
            } => vk_standard_3d_textured_descr_set,
        }
    }

    pub fn unwrap_2d_descr(&self) -> &Arc<DescriptorSets> {
        match self {
            TextureData::Tex2D {
                vk_standard_textured_descr_set,
                ..
            } => vk_standard_textured_descr_set,
            TextureData::Tex3D { .. } => panic!("not a 2d texture"),
        }
    }
}

#[derive(Debug, Hiarc)]
pub struct CTexture {
    pub data: TextureData,

    pub width: usize,
    pub height: usize,
    pub depth: usize,

    pub mip_map_count: u32,
}

#[derive(Debug, Hiarc)]
pub struct BufferObjectMem {
    pub mem: Arc<MemoryBlock>,
}

#[derive(Debug, Hiarc)]
pub struct BufferObject {
    pub buffer_object: BufferObjectMem,

    pub cur_buffer: Arc<Buffer>,
    pub cur_buffer_offset: usize,
}

#[derive(Debug, Hiarc)]
pub struct StreamedUniformBuffer {
    pub uniform_sets: [Arc<DescriptorSet>; 2],
}

#[derive(Debug)]
pub struct ShaderModule {
    pub vert_shader_module: vk::ShaderModule,
    pub frag_shader_module: vk::ShaderModule,

    vk_device: Arc<LogicalDevice>,
}

impl ShaderModule {
    pub fn new(
        vert_shader_module: vk::ShaderModule,
        frag_shader_module: vk::ShaderModule,
        vk_device: &Arc<LogicalDevice>,
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

#[derive(Debug, Hiarc, FromPrimitive, Copy, Clone, PartialEq)]
#[repr(u32)]
pub enum EVulkanBackendBlendModes {
    Alpha = 0,
    None = 1,
    Additive = 2,
}
pub const BLEND_MODE_COUNT: usize = 3;

#[derive(Debug, Hiarc, FromPrimitive, Copy, Clone, PartialEq)]
#[repr(u32)]
pub enum EVulkanBackendClipModes {
    None = 0,
    DynamicScissorAndViewport = 1,

    Count = 2,
}

const MAX_TEXTURE_MODES: usize = 2;

#[derive(Debug, Hiarc, Default, Copy, Clone, PartialEq)]
pub enum RenderPassSubType {
    #[default]
    Single = 0,
    // switch around 2 framebuffers to use each other as
    // input attachments
    Switching1,
    Switching2,
}

#[derive(Debug, Hiarc, Copy, Clone, PartialEq)]
pub enum RenderPassType {
    Normal(RenderPassSubType),
    MultiSampling,
}

impl Default for RenderPassType {
    fn default() -> Self {
        Self::Normal(Default::default())
    }
}

#[derive(Debug, Hiarc, Clone)]
pub struct PipelineCreationAttributesEx {
    pub address_mode_index: usize,
    pub is_textured: bool,
}

#[derive(Debug, Hiarc, Clone)]
pub struct PipelineCreationProps {
    pub attr: PipelineCreationAttributes,
    pub attr_ex: PipelineCreationAttributesEx,
}

#[derive(Debug, Hiarc, Clone)]
pub struct PipelineCreationOneByOne {
    pub multi_sampling_count: u32,
    pub device: Arc<LogicalDevice>,
    pub shader_compiler: Arc<ShaderCompiler>,
    #[hiarc_skip_unsafe]
    pub swapchain_extent: vk::Extent2D,
    #[hiarc_skip_unsafe]
    pub render_pass: vk::RenderPass,

    pub pipeline_cache: Option<Arc<PipelineCacheInner>>,
}

#[derive(Debug, Hiarc, Default)]
pub enum PipelineContainerItem {
    Normal {
        pipeline: Pipelines,
    },
    MaybeUninit {
        #[hiarc_skip_unsafe]
        pipeline_and_layout: parking_lot::Mutex<Option<Pipelines>>,

        creation_props: PipelineCreationProps,
        creation_data: PipelineCreationOneByOne,
    },
    #[default]
    None,
}

#[derive(Debug, Hiarc, Clone)]
pub enum PipelineContainerCreateMode {
    AtOnce,
    OneByOne(PipelineCreationOneByOne),
}

#[derive(Debug, Hiarc)]
pub struct PipelineContainer {
    // 3 blend modes - 2 viewport & scissor modes - 2 texture modes - 4 stencil modes - 3 color mask types - 3 sampler modes
    pub pipelines: [[[[[[Box<PipelineContainerItem>; SAMPLER_TYPES_COUNT]; COLOR_MASK_TYPE_COUNT];
        STENCIL_OP_TYPE_COUNT]; MAX_TEXTURE_MODES];
        EVulkanBackendClipModes::Count as usize]; BLEND_MODE_COUNT],

    pub(crate) mode: PipelineContainerCreateMode,
}

impl PipelineContainer {
    pub fn new(mode: PipelineContainerCreateMode) -> Self {
        Self {
            pipelines: Default::default(),
            mode,
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

impl From<ESupportedSamplerTypes> for SamplerAddressMode {
    fn from(val: ESupportedSamplerTypes) -> Self {
        match val {
            ESupportedSamplerTypes::Repeat => SamplerAddressMode::Repeat,
            ESupportedSamplerTypes::ClampToEdge => SamplerAddressMode::ClampToEdge,
            ESupportedSamplerTypes::Texture2DArray => SamplerAddressMode::Texture2DArray,
        }
    }
}

#[derive(Debug, Hiarc)]
pub struct SwapChainImageBase {
    pub image: Arc<Image>,
    pub img_mem: MemoryImageBlock,
    pub img_view: Arc<ImageView>,
}

#[derive(Debug, Hiarc)]
pub struct SwapChainImageFull {
    pub base: SwapChainImageBase,

    pub texture_descr_sets: Arc<DescriptorSets>,
}

#[derive(Debug, Hiarc, Default)]
pub struct ThreadCommandGroup {
    pub render_pass: RenderPassType,

    pub render_pass_index: usize,
    pub canvas_index: FrameCanvasIndex,

    pub cur_frame_index: u32,

    pub in_order_id: usize,

    pub cmds: Vec<RenderCommandExecuteBuffer>,
}

#[derive(Debug, Hiarc)]
pub enum RenderThreadEvent {
    ClearFrame(u32),
    ClearFrames,
}

#[derive(Debug, Hiarc)]
pub struct RenderThreadInner {
    pub thread: Option<std::thread::JoinHandle<()>>,
    pub finished: bool,
    pub started: bool,

    pub events: Vec<RenderThreadEvent>,
    pub render_calls: Vec<(ThreadCommandGroup, Arc<CanvasSetup>)>,
}

#[derive(Debug, Hiarc)]
pub struct RenderThread {
    pub inner: parking_lot::Mutex<RenderThreadInner>,
    pub cond: parking_lot::Condvar,
}
