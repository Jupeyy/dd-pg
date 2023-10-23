use std::sync::{atomic::AtomicU32, Arc, Condvar, Mutex};

use ash::vk;
use graphics_types::{command_buffer::CommandsRender, rendering::WRAP_TYPE_COUNT};
use num_derive::FromPrimitive;

use super::{
    buffer::Buffer,
    descriptor_pool::DescriptorPool,
    descriptor_set::DescriptorSet,
    image::Image,
    image_view::ImageView,
    mapped_memory::MappedMemory,
    memory::{SMemoryBlock, SMemoryImageBlock},
    memory_block::SDeviceMemoryBlock,
    vulkan_allocator::{IMAGE_BUFFER_CACHE_ID, VERTEX_BUFFER_CACHE_ID},
};

/*
static void dbg_msg(const char *sys, const char *fmt, ...)
{
    va_list args;
    va_start(args, fmt);
    std::vprintf(fmt, args);
    va_end(args);
}
static constexpr const char *Localizable(const char *pStr) { return pStr; }

static void mem_copy(void *dest, const void *source, unsigned size) { memcpy(dest, source, size); }
static void dbg_break()
{
#ifdef __GNUC__
    __builtin_trap();
#else
    abort();
#endif
}
static void dbg_assert(int test, const char *msg)
{
    if(!test)
    {
        dbg_msg("assert", "%s", msg);
        dbg_break();
    }
}
static int str_comp(const char *a, const char *b) { return strcmp(a, b); }
static void str_copy(char *dst, const char *src, int dst_size)
{
    dst[0] = '\0';
    strncat(dst, src, dst_size - 1);
}
template<int N>
static void str_copy(char (&dst)[N], const char *src)
{
    str_copy(dst, src, N);
}
static int str_format(char *buffer, int buffer_size, const char *format, ...)
{
#if defined(CONF_FAMILY_WINDOWS)
    va_list ap;
    va_start(ap, format);
    _vsnprintf(buffer, buffer_size, format, ap);
    va_end(ap);

    buffer[buffer_size - 1] = 0; /* assure null termination */
#else
    va_list ap;
    va_start(ap, format);
    vsnprintf(buffer, buffer_size, format, ap);
    va_end(ap);

    /* null termination is assured by definition of vsnprintf */
#endif
    return 1;
}*/

/*
static_assert(std::chrono::steady_clock::is_steady, "Compiler does not support steady clocks, it might be out of date.");
static_assert(std::chrono::steady_clock::period::den / std::chrono::steady_clock::period::num >= 1000000000, "Compiler has a bad timer precision and might be out of date.");
static const std::chrono::time_point<std::chrono::steady_clock> tw_start_time = std::chrono::steady_clock::now();

int64_t time_get_impl() { return std::chrono::duration_cast<std::chrono::nanoseconds>(std::chrono::steady_clock::now() - tw_start_time).count(); }

std::chrono::nanoseconds time_get_nanoseconds() { return std::chrono::nanoseconds(time_get_impl()); }
-----  time ----- */

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

#[derive(Debug, Default, Clone)]
pub struct SDeviceDescriptorPools {
    pub pools: Vec<Arc<DescriptorPool>>,
    pub default_alloc_size: vk::DeviceSize,
    pub is_uniform_pool: bool,
}

#[derive(Debug)]
pub enum TextureData {
    Tex2D {
        img: Arc<Image>,
        img_mem: SMemoryImageBlock<IMAGE_BUFFER_CACHE_ID>,
        img_view: Arc<ImageView>,
        samplers: [vk::Sampler; WRAP_TYPE_COUNT],

        vk_standard_textured_descr_sets: [Arc<DescriptorSet>; WRAP_TYPE_COUNT],
    },
    Tex3D {
        img_3d: Arc<Image>,
        img_3d_mem: SMemoryImageBlock<IMAGE_BUFFER_CACHE_ID>,
        img_3d_view: Arc<ImageView>,
        sampler_3d: vk::Sampler,

        vk_standard_3d_textured_descr_set: Arc<DescriptorSet>,
    },
}

impl TextureData {
    pub fn unwrap_3d_descr(&self) -> &Arc<DescriptorSet> {
        match self {
            TextureData::Tex2D { .. } => panic!("not a 3d texture"),
            TextureData::Tex3D {
                vk_standard_3d_textured_descr_set,
                ..
            } => vk_standard_3d_textured_descr_set,
        }
    }

    pub fn unwrap_2d_descr(&self, index: usize) -> &Arc<DescriptorSet> {
        match self {
            TextureData::Tex2D {
                vk_standard_textured_descr_sets,
                ..
            } => &vk_standard_textured_descr_sets[index],
            TextureData::Tex3D { .. } => panic!("not a 2d texture"),
        }
    }

    pub fn unwrap_2d_descrs(&self) -> &[Arc<DescriptorSet>; WRAP_TYPE_COUNT] {
        match self {
            TextureData::Tex2D {
                vk_standard_textured_descr_sets,
                ..
            } => &vk_standard_textured_descr_sets,
            TextureData::Tex3D { .. } => panic!("not a 2d texture"),
        }
    }

    pub fn unwrap_img_view_2d(&self) -> vk::ImageView {
        match self {
            TextureData::Tex2D { img_view, .. } => img_view.image_view,
            TextureData::Tex3D { .. } => panic!("not a 2d texture"),
        }
    }

    pub fn unwrap_img_2d_unsafe(&self) -> vk::Image {
        match self {
            TextureData::Tex2D { img, .. } => img.image,
            TextureData::Tex3D { .. } => panic!("not a 2d texture"),
        }
    }

    pub fn unwrap_img_2d(&self) -> &Arc<Image> {
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

/*impl Default for CTexture {
    fn default() -> Self {
        Self {
            img: Default::default(),
            img_mem: Default::default(),
            img_view: Default::default(),
            samplers: Default::default(),
            img_3d: Default::default(),
            img_3d_mem: Default::default(),
            img_3d_view: Default::default(),
            sampler_3d: Default::default(),
            width: Default::default(),
            height: Default::default(),
            depth: Default::default(),
            rescale_count: Default::default(),
            mip_map_count: 1,
            vk_standard_textured_descr_sets: Default::default(),
            vk_standard_3d_textured_descr_set: Default::default(),
            vk_text_descr_set: Default::default(),
        }
    }
}*/

#[derive(Debug)]
pub struct SBufferObject {
    pub mem: SMemoryBlock<VERTEX_BUFFER_CACHE_ID>,
}

#[derive(Debug)]
pub struct SBufferObjectFrame {
    pub buffer_object: SBufferObject,

    pub cur_buffer: Arc<Buffer>,
    pub cur_buffer_offset: usize,
}

#[derive(Debug, Clone)]
pub struct SFrameBuffers {
    pub buffer: Arc<Buffer>,
    pub buffer_mem: Arc<SDeviceMemoryBlock>,
    pub offset_in_buffer: usize,
    pub size: usize,
    pub used_size: usize,
    pub is_used: bool,
    pub mapped_buffer_data: Option<(isize, Arc<MappedMemory>)>,
}

impl SFrameBuffers {
    pub fn new(buffer: Arc<Buffer>, buffer_mem: Arc<SDeviceMemoryBlock>) -> Self {
        Self {
            buffer,
            buffer_mem,
            offset_in_buffer: Default::default(),
            size: Default::default(),
            used_size: Default::default(),
            is_used: Default::default(),
            mapped_buffer_data: None,
        }
    }
}

impl StreamMemory for SFrameBuffers {
    fn get_device_mem_block(&self) -> &Arc<SDeviceMemoryBlock> {
        &self.buffer_mem
    }

    fn get_buffer(&self) -> &Arc<Buffer> {
        &self.buffer
    }

    fn get_offset_in_buffer(&mut self) -> &mut usize {
        &mut self.offset_in_buffer
    }

    fn get_used_size(&mut self) -> &mut usize {
        &mut self.used_size
    }

    fn get_is_used(&mut self) -> &mut bool {
        &mut self.is_used
    }

    fn get_size(&mut self) -> &mut usize {
        &mut self.size
    }

    fn get_mapped_buffer_data(&self) -> (isize, &Arc<MappedMemory>) {
        let mem = self.mapped_buffer_data.as_ref().unwrap();
        (mem.0, &mem.1)
    }

    fn get(&mut self) -> &mut SFrameBuffers {
        self
    }

    fn new(
        buffer: Arc<Buffer>,
        buffer_mem: Arc<SDeviceMemoryBlock>,
        offset_in_buffer: usize,
        size: usize,
        used_size: usize,
        mapped_buffer_data: (isize, Arc<MappedMemory>),
    ) -> Self {
        Self {
            buffer,
            buffer_mem,
            offset_in_buffer,
            size,
            used_size,
            is_used: Default::default(),
            mapped_buffer_data: Some(mapped_buffer_data),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SFrameUniformBuffers {
    pub base: SFrameBuffers,
    pub uniform_sets: [Option<Arc<DescriptorSet>>; 2],
}

impl StreamMemory for SFrameUniformBuffers {
    fn get_device_mem_block(&self) -> &Arc<SDeviceMemoryBlock> {
        &self.base.buffer_mem
    }

    fn get_buffer(&self) -> &Arc<Buffer> {
        &self.base.buffer
    }

    fn get_offset_in_buffer(&mut self) -> &mut usize {
        &mut self.base.offset_in_buffer
    }

    fn get_used_size(&mut self) -> &mut usize {
        &mut self.base.used_size
    }

    fn get_is_used(&mut self) -> &mut bool {
        &mut self.base.is_used
    }

    fn get_size(&mut self) -> &mut usize {
        &mut self.base.size
    }

    fn get_mapped_buffer_data(&self) -> (isize, &Arc<MappedMemory>) {
        self.base.get_mapped_buffer_data()
    }

    fn get(&mut self) -> &mut SFrameBuffers {
        &mut self.base
    }

    fn new(
        buffer: Arc<Buffer>,
        buffer_mem: Arc<SDeviceMemoryBlock>,
        offset_in_buffer: usize,
        size: usize,
        used_size: usize,
        mapped_buffer_data: (isize, Arc<MappedMemory>),
    ) -> Self {
        Self {
            base: SFrameBuffers {
                buffer,
                buffer_mem,
                offset_in_buffer,
                size,
                used_size,
                is_used: Default::default(),
                mapped_buffer_data: Some(mapped_buffer_data),
            },
            uniform_sets: Default::default(),
        }
    }
}

type TBufferObjectsOfFrame<TName> = Vec<TName>;
type TMemoryMapRangesOfFrame = Vec<vk::MappedMemoryRange>;

#[derive(Debug, Clone)]
pub struct SStreamMemoryOfFrame<TName: Clone> {
    pub buffer_objects_of_frames: TBufferObjectsOfFrame<TName>,
    pub buffer_objects_of_frames_range_datas: TMemoryMapRangesOfFrame,
    pub current_used_count: usize,
}

// vk::MappedMemoryRange contains a pointer
unsafe impl<TName: Clone> Send for SStreamMemoryOfFrame<TName> {}

impl<TName: Clone + std::fmt::Debug> Default for SStreamMemoryOfFrame<TName> {
    fn default() -> Self {
        Self {
            buffer_objects_of_frames: Default::default(),
            buffer_objects_of_frames_range_datas: Default::default(),
            current_used_count: Default::default(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SStreamMemory<TName: Clone + std::fmt::Debug> {
    stream_memory_of_frame: Vec<SStreamMemoryOfFrame<TName>>,
}

impl<TName: Clone + std::fmt::Debug> Default for SStreamMemory<TName> {
    fn default() -> Self {
        Self {
            stream_memory_of_frame: Default::default(),
        }
    }
}

pub trait StreamMemory {
    fn get_device_mem_block(&self) -> &Arc<SDeviceMemoryBlock>;
    fn get_buffer(&self) -> &Arc<Buffer>;
    fn get_offset_in_buffer(&mut self) -> &mut usize;
    fn get_used_size(&mut self) -> &mut usize;
    fn get_is_used(&mut self) -> &mut bool;
    fn reset_is_used(&mut self) {
        *self.get_used_size() = 0;
        *self.get_is_used() = false;
    }
    fn set_is_used(&mut self) {
        *self.get_is_used() = true;
    }
    fn get_size(&mut self) -> &mut usize;
    /// returns the mapped memory and it's offset
    fn get_mapped_buffer_data(&self) -> (isize, &Arc<MappedMemory>);
    fn get(&mut self) -> &mut SFrameBuffers;
    fn new(
        buffer: Arc<Buffer>,
        buffer_mem: Arc<SDeviceMemoryBlock>,
        offset_in_buffer: usize,
        size: usize,
        used_size: usize,
        mapped_buffer_data: (isize, Arc<MappedMemory>),
    ) -> Self;
}

impl<TName> SStreamMemory<TName>
where
    TName: Clone + StreamMemory + std::fmt::Debug,
{
    pub fn get_buffers(&mut self, frame_image_index: usize) -> &mut Vec<TName> {
        &mut self.stream_memory_of_frame[frame_image_index].buffer_objects_of_frames
    }

    pub fn get_ranges(&mut self, frame_image_index: usize) -> &mut Vec<vk::MappedMemoryRange> {
        &mut self.stream_memory_of_frame[frame_image_index].buffer_objects_of_frames_range_datas
    }

    pub fn get_buffers_and_ranges(
        &mut self,
        frame_image_index: usize,
    ) -> (&mut Vec<TName>, &mut Vec<vk::MappedMemoryRange>) {
        let stream_mem = &mut self.stream_memory_of_frame[frame_image_index];
        (
            &mut stream_mem.buffer_objects_of_frames,
            &mut stream_mem.buffer_objects_of_frames_range_datas,
        )
    }

    pub fn get_used_count(&self, frame_image_index: usize) -> usize {
        self.stream_memory_of_frame[frame_image_index].current_used_count
    }

    pub fn increase_used_count(&mut self, frame_image_index: usize) {
        self.stream_memory_of_frame[frame_image_index].current_used_count += 1;
    }

    pub fn get_current_buffer(&mut self, frame_image_index: usize) -> &mut TName {
        let cur_count = self.stream_memory_of_frame[frame_image_index].current_used_count;
        &mut self.stream_memory_of_frame[frame_image_index].buffer_objects_of_frames[cur_count - 1]
    }

    #[must_use]
    pub fn is_used(&self, frame_image_index: usize) -> bool {
        self.get_used_count(frame_image_index) > 0
    }

    pub fn reset_frame(&mut self, frame_image_index: usize) {
        self.stream_memory_of_frame[frame_image_index].current_used_count = 0;
    }

    pub fn init(&mut self, frame_image_count: usize) {
        self.stream_memory_of_frame
            .resize(frame_image_count, Default::default());
    }

    pub fn destroy<T>(&mut self, destroy_buffer: &mut T)
    where
        T: FnMut(usize, &mut TName),
    {
        let mut image_index: usize = 0;
        for memory_of_frame in &mut self.stream_memory_of_frame {
            let buffers_of_frame = &mut memory_of_frame.buffer_objects_of_frames;
            for i in 0..buffers_of_frame.len() {
                let buffer_of_frame = buffers_of_frame.get_mut(i).unwrap();
                destroy_buffer(image_index, buffer_of_frame);
            }
            image_index += 1;
        }
        self.stream_memory_of_frame.clear();
    }
}

pub struct SShaderModule {
    pub vert_shader_module: vk::ShaderModule,
    pub frag_shader_module: vk::ShaderModule,

    pub vk_device: ash::Device,
}

impl SShaderModule {
    pub fn new(vk_device: &ash::Device) -> Self {
        Self {
            vert_shader_module: Default::default(),
            frag_shader_module: Default::default(),
            vk_device: vk_device.clone(),
        }
    }
}

impl Drop for SShaderModule {
    fn drop(&mut self) {
        if self.vk_device.handle() != vk::Device::null() {
            if self.vert_shader_module != vk::ShaderModule::null() {
                unsafe {
                    self.vk_device
                        .destroy_shader_module(self.vert_shader_module, None);
                }
            }

            if self.frag_shader_module != vk::ShaderModule::null() {
                unsafe {
                    self.vk_device
                        .destroy_shader_module(self.frag_shader_module, None);
                }
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

#[derive(FromPrimitive, Copy, Clone, PartialEq)]
#[repr(u32)]
pub enum EVulkanBackendBlendModes {
    Alpha = 0,
    None = 1,
    Additative = 2,

    Count = 3,
}

#[derive(FromPrimitive, Copy, Clone, PartialEq)]
#[repr(u32)]
pub enum EVulkanBackendClipModes {
    None = 0,
    DynamicScissorAndViewport = 1,

    Count = 2,
}

#[derive(PartialEq, Clone, Copy)]
pub enum EVulkanBackendTextureModes {
    NotTextured = 0,
    Textured,

    Count,
}

#[derive(Debug, Default, Copy, Clone, FromPrimitive)]
pub enum RenderPassType {
    #[default]
    Single = 0,
    // switch around 2 framebuffers to use each other as
    // input attachments
    Switching1,
    Switching2,
}

pub const RENDER_PASS_TYPE_COUNT: usize = 3;

#[derive(Debug, Default, Clone)]
pub struct PipelineContainer {
    // 3 blend modes - 2 viewport & scissor modes - 2 texture modes
    pub pipeline_layouts: [[[vk::PipelineLayout; EVulkanBackendTextureModes::Count as usize];
        EVulkanBackendClipModes::Count as usize];
        EVulkanBackendBlendModes::Count as usize],
    pub pipelines: [[[vk::Pipeline; EVulkanBackendTextureModes::Count as usize];
        EVulkanBackendClipModes::Count as usize];
        EVulkanBackendBlendModes::Count as usize],
}

impl PipelineContainer {
    pub fn destroy(&mut self, device: &ash::Device) {
        for pipe_layouts_list in &mut self.pipeline_layouts {
            for pipe_layouts in pipe_layouts_list {
                for pipe_layout in pipe_layouts {
                    if *pipe_layout != vk::PipelineLayout::null() {
                        unsafe {
                            device.destroy_pipeline_layout(*pipe_layout, None);
                        }
                    }
                    *pipe_layout = vk::PipelineLayout::null();
                }
            }
        }
        for pipe_list in &mut self.pipelines {
            for pipes in pipe_list {
                for pipe in pipes {
                    if *pipe != vk::Pipeline::null() {
                        unsafe {
                            device.destroy_pipeline(*pipe, None);
                        }
                    }
                    *pipe = vk::Pipeline::null();
                }
            }
        }
    }
}

pub enum ESupportedSamplerTypes {
    Repeat = 0,
    ClampToEdge,
    Texture2DArray,

    Count,
}

#[derive(Debug)]
pub struct SShaderFileCache {
    pub binary: Vec<u8>,
}

#[derive(Debug, Default)]
pub struct SSwapImgViewportExtent {
    pub swap_image_viewport: vk::Extent2D,
    pub has_forced_viewport: bool,
    pub forced_viewport: vk::Extent2D,
}

impl SSwapImgViewportExtent {
    // the viewport of the resulting presented image on the screen
    // if there is a forced viewport the resulting image is smaller
    // than the full swap image size
    pub fn get_presented_image_viewport(&self) -> vk::Extent2D {
        let mut viewport_width = self.swap_image_viewport.width;
        let mut viewport_height = self.swap_image_viewport.height;
        if self.has_forced_viewport {
            viewport_width = self.forced_viewport.width;
            viewport_height = self.forced_viewport.height;
        }

        vk::Extent2D {
            width: viewport_width,
            height: viewport_height,
        }
    }
}

#[derive(Debug)]
pub struct SwapChainImageBase {
    pub image: Arc<Image>,
    pub img_mem: SMemoryImageBlock<IMAGE_BUFFER_CACHE_ID>,
    pub img_view: Arc<ImageView>,

    pub layout_is_undefined: bool,
}

#[derive(Debug)]
pub struct SwapChainImageFull {
    pub base: SwapChainImageBase,

    pub samplers: [vk::Sampler; WRAP_TYPE_COUNT],

    pub texture_descr_sets: [Arc<DescriptorSet>; WRAP_TYPE_COUNT],
}

#[derive(Debug, Clone)]
pub struct VKDelayedBufferCleanupItem {
    pub buffer: Option<Arc<Buffer>>,
    pub mem: Arc<SDeviceMemoryBlock>,
    pub mapped_data: Option<(isize, Arc<MappedMemory>)>,
}

#[derive(Debug, Default)]
pub struct TThreadCommandGroup {
    pub render_pass: RenderPassType,

    pub render_pass_index: usize,

    pub cur_frame_index: u32,

    pub in_order_id: usize,

    pub cmds: Vec<RenderCommandExecuteBuffer>,
}

#[derive(Debug)]
pub struct SRenderThreadInner {
    pub is_rendering: bool,
    pub thread: Option<std::thread::JoinHandle<()>>,
    pub finished: bool,
    pub started: bool,

    pub command_groups: Vec<TThreadCommandGroup>,

    pub next_frame_index: Arc<AtomicU32>,
    pub next_frame_count: Arc<AtomicU32>,
}

#[derive(Debug)]
pub struct SRenderThread {
    pub inner: Mutex<SRenderThreadInner>,
    pub cond: Condvar,
}

#[derive(Debug)]
pub struct RenderCommandExecuteBuffer {
    pub raw_render_command: Option<CommandsRender>,

    // must be calculated when the buffer gets filled
    pub estimated_render_call_count: usize,

    // useful data
    pub buffer: vk::Buffer,
    pub buffer_off: usize,

    // up to two descriptors are supported
    pub descriptors: [Option<Arc<DescriptorSet>>; 2],

    pub index_buffer: vk::Buffer,

    pub clear_color_in_render_thread: bool,

    pub viewport_size: vk::Extent2D,

    pub has_dynamic_state: bool,
    pub viewport: vk::Viewport,
    pub scissor: vk::Rect2D,
}

impl Default for RenderCommandExecuteBuffer {
    fn default() -> Self {
        Self {
            raw_render_command: Default::default(),
            estimated_render_call_count: Default::default(),
            buffer: Default::default(),
            buffer_off: Default::default(),
            descriptors: Default::default(),
            index_buffer: Default::default(),
            clear_color_in_render_thread: Default::default(),

            viewport_size: Default::default(),

            has_dynamic_state: false,
            scissor: Default::default(),
            viewport: Default::default(),
        }
    }
}

unsafe impl Send for RenderCommandExecuteBuffer {}
