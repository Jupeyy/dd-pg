use std::{
    collections::BTreeMap,
    sync::{
        atomic::{AtomicU64, AtomicU8},
        Arc, Mutex,
    },
};

use ash::vk;
use graphics_types::command_buffer::{GL_SVertexTex3DStream, StreamDataMax};
use libc::c_void;

use super::{
    common::{localizable, EGFXErrorType},
    vulkan_allocator::VulkanAllocator,
    vulkan_dbg::is_verbose,
    vulkan_error::Error,
    vulkan_limits::Limits,
    vulkan_mem::Memory,
    vulkan_types::{
        CTexture, EMemoryBlockUsage, ESupportedSamplerTypes, SBufferContainer, SBufferObjectFrame,
        SDelayedBufferCleanupItem, SDeviceDescriptorPool, SDeviceDescriptorPools,
        SDeviceDescriptorSet, SDeviceMemoryBlock, SFrameBuffers, SFrameUniformBuffers,
        SMemoryBlock, SMemoryBlockCache, SMemoryHeapQueueElement, SMemoryImageBlock, SStreamMemory,
        StreamMemory, IMAGE_BUFFER_CACHE_ID, STAGING_BUFFER_CACHE_ID,
        STAGING_BUFFER_IMAGE_CACHE_ID, VERTEX_BUFFER_CACHE_ID,
    },
};

// good approximation of 1024x1024 image with mipmaps
const IMG_SIZE1024X1024: i64 = (1024 * 1024 * 4) * 2;

pub struct Device {
    dbg: Arc<AtomicU8>, // @see EDebugGFXModes
    pub mem: Memory,
    pub mem_allocator: Arc<std::sync::Mutex<Option<VulkanAllocator>>>,

    instance: ash::Instance,
    pub device: ash::Device,
    error: Arc<std::sync::Mutex<Error>>,
    pub vk_gpu: vk::PhysicalDevice,

    pub staging_buffer_cache: SMemoryBlockCache<{ STAGING_BUFFER_CACHE_ID }>,
    pub staging_buffer_cache_image: SMemoryBlockCache<{ STAGING_BUFFER_IMAGE_CACHE_ID }>,
    pub vertex_buffer_cache: SMemoryBlockCache<{ VERTEX_BUFFER_CACHE_ID }>,
    pub image_buffer_caches: BTreeMap<u32, SMemoryBlockCache<{ IMAGE_BUFFER_CACHE_ID }>>,

    pub texture_memory_usage: Arc<AtomicU64>,
    pub buffer_memory_usage: Arc<AtomicU64>,
    pub stream_memory_usage: Arc<AtomicU64>,
    pub staging_memory_usage: Arc<AtomicU64>,

    pub limits: Limits,

    pub non_flushed_staging_buffer_ranges: Vec<vk::MappedMemoryRange>,

    pub frame_delayed_buffer_cleanups: Vec<Vec<SDelayedBufferCleanupItem>>,
    pub frame_delayed_texture_cleanups: Vec<Vec<CTexture>>,
    pub frame_delayed_text_textures_cleanups: Vec<Vec<(CTexture, CTexture)>>,

    pub swap_chain_image_count: u32,

    pub samplers: [vk::Sampler; ESupportedSamplerTypes::Count as usize],
    pub textures: Vec<CTexture>,

    pub streamed_vertex_buffer: SStreamMemory<SFrameBuffers>,

    pub streamed_uniform_buffers: Vec<SStreamMemory<SFrameUniformBuffers>>,

    pub buffer_objects: Vec<SBufferObjectFrame>,
    pub buffer_containers: Vec<SBufferContainer>,

    pub standard_texture_descr_pool: SDeviceDescriptorPools,
    pub text_texture_descr_pool: SDeviceDescriptorPools,

    pub uniform_buffer_descr_pools: Vec<SDeviceDescriptorPools>,

    pub standard_textured_descriptor_set_layout: vk::DescriptorSetLayout,
    pub standard_3d_textured_descriptor_set_layout: vk::DescriptorSetLayout,

    pub text_descriptor_set_layout: vk::DescriptorSetLayout,

    pub sprite_multi_uniform_descriptor_set_layout: vk::DescriptorSetLayout,
    pub quad_uniform_descriptor_set_layout: vk::DescriptorSetLayout,

    // command buffers
    pub memory_command_buffers: Vec<vk::CommandBuffer>,
    pub used_memory_command_buffer: Vec<bool>,

    // device props
    pub allows_linear_blitting: bool,
    pub optimal_swap_chain_image_blitting: bool,
    pub optimal_rgba_image_blitting: bool,
    pub linear_rgba_image_blitting: bool,

    pub global_texture_lod_bias: i32,
}

impl Device {
    pub fn new(
        dbg: Arc<AtomicU8>, // @see EDebugGFXModes
        instance: &ash::Instance,
        device: &ash::Device,
        error: Arc<std::sync::Mutex<Error>>,
        vk_gpu: vk::PhysicalDevice,

        texture_memory_usage: Arc<AtomicU64>,
        buffer_memory_usage: Arc<AtomicU64>,
        stream_memory_usage: Arc<AtomicU64>,
        staging_memory_usage: Arc<AtomicU64>,

        limits: Limits,
    ) -> Self {
        Self {
            dbg: dbg.clone(),
            mem: Memory::new(
                dbg.clone(),
                error.clone(),
                &instance.clone(),
                &device.clone(),
                vk_gpu.clone(),
                texture_memory_usage.clone(),
                buffer_memory_usage.clone(),
                stream_memory_usage.clone(),
                staging_memory_usage.clone(),
            ),
            mem_allocator: Arc::new(std::sync::Mutex::new(Some(VulkanAllocator::new(
                Memory::new(
                    dbg,
                    error.clone(),
                    instance,
                    device,
                    vk_gpu,
                    texture_memory_usage.clone(),
                    buffer_memory_usage.clone(),
                    stream_memory_usage.clone(),
                    staging_memory_usage.clone(),
                ),
                limits.clone(),
            )))),
            instance: instance.clone(),
            device: device.clone(),
            error: error,
            vk_gpu,
            staging_buffer_cache: Default::default(),
            staging_buffer_cache_image: Default::default(),
            vertex_buffer_cache: Default::default(),
            image_buffer_caches: Default::default(),
            texture_memory_usage,
            buffer_memory_usage,
            stream_memory_usage,
            staging_memory_usage,
            limits: limits,
            non_flushed_staging_buffer_ranges: Default::default(),
            frame_delayed_buffer_cleanups: Default::default(),
            frame_delayed_texture_cleanups: Default::default(),
            frame_delayed_text_textures_cleanups: Default::default(),
            swap_chain_image_count: Default::default(),
            samplers: Default::default(),
            textures: Default::default(),
            streamed_vertex_buffer: Default::default(),
            streamed_uniform_buffers: Default::default(),
            buffer_objects: Default::default(),
            buffer_containers: Default::default(),
            standard_texture_descr_pool: Default::default(),
            text_texture_descr_pool: Default::default(),
            uniform_buffer_descr_pools: Default::default(),
            standard_textured_descriptor_set_layout: Default::default(),
            standard_3d_textured_descriptor_set_layout: Default::default(),
            text_descriptor_set_layout: Default::default(),
            sprite_multi_uniform_descriptor_set_layout: Default::default(),
            quad_uniform_descriptor_set_layout: Default::default(),
            memory_command_buffers: Default::default(),
            used_memory_command_buffer: Default::default(),
            allows_linear_blitting: Default::default(),
            optimal_swap_chain_image_blitting: Default::default(),
            optimal_rgba_image_blitting: Default::default(),
            linear_rgba_image_blitting: Default::default(),
            global_texture_lod_bias: Default::default(),
        }
    }

    /************************
     * MEMORY MANAGEMENT
     ************************/
    #[must_use]
    pub fn get_staging_buffer(
        &mut self,
        res_block: &mut SMemoryBlock<STAGING_BUFFER_CACHE_ID>,
        buffer_data: *const c_void,
        required_size: vk::DeviceSize,
    ) -> bool {
        return self
            .mem
            .GetBufferBlockImpl::<{ STAGING_BUFFER_CACHE_ID }, { 8 * 1024 * 1024 }, 3, true>(
                res_block,
                &mut self.staging_buffer_cache,
                vk::BufferUsageFlags::TRANSFER_SRC,
                vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_CACHED,
                buffer_data,
                required_size,
                std::cmp::max::<vk::DeviceSize>(self.limits.non_coherent_mem_alignment, 16),
            );
    }

    #[must_use]
    pub fn get_staging_buffer_image(
        mem: &mut Memory,
        staging_buffer_cache: &mut SMemoryBlockCache<{ STAGING_BUFFER_IMAGE_CACHE_ID }>,
        limits: &Limits,
        res_block: &mut SMemoryBlock<STAGING_BUFFER_IMAGE_CACHE_ID>,
        buffer_data: &[u8],
        required_size: vk::DeviceSize,
    ) -> bool {
        return mem
            .GetBufferBlockImpl::<{ STAGING_BUFFER_IMAGE_CACHE_ID }, { 8 * 1024 * 1024 }, 3, true>(
                res_block,
                staging_buffer_cache,
                vk::BufferUsageFlags::TRANSFER_SRC,
                vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_CACHED,
                buffer_data.as_ptr() as *const c_void,
                required_size,
                std::cmp::max::<vk::DeviceSize>(
                    limits.optimal_image_copy_mem_alignment,
                    std::cmp::max::<vk::DeviceSize>(limits.non_coherent_mem_alignment, 16),
                ),
            );
    }

    #[must_use]
    pub fn GetImageMemory(
        &mut self,
        RetBlock: &mut SMemoryImageBlock<IMAGE_BUFFER_CACHE_ID>,
        RequiredSize: vk::DeviceSize,
        RequiredAlignment: vk::DeviceSize,
        RequiredMemoryTypeBits: u32,
    ) -> bool {
        let mut it = self.image_buffer_caches.get_mut(&RequiredMemoryTypeBits);
        let mem: &mut SMemoryBlockCache<{ IMAGE_BUFFER_CACHE_ID }>;
        if it.is_none() {
            self.image_buffer_caches
                .insert(RequiredMemoryTypeBits, SMemoryBlockCache::default());
            it = self.image_buffer_caches.get_mut(&RequiredMemoryTypeBits);

            mem = it.unwrap();
            mem.init(self.swap_chain_image_count as usize);
        } else {
            mem = it.unwrap();
        }
        return self
            .mem
            .GetImageMemoryBlockImpl::<IMAGE_BUFFER_CACHE_ID, IMG_SIZE1024X1024, 2>(
                RetBlock,
                mem,
                vk::MemoryPropertyFlags::DEVICE_LOCAL,
                RequiredSize,
                RequiredAlignment,
                RequiredMemoryTypeBits,
            );
    }

    pub fn FreeImageMemBlock(
        frame_delay_buffer_cleanup: &mut Vec<Vec<SDelayedBufferCleanupItem>>,
        image_buffer_caches: &mut BTreeMap<u32, SMemoryBlockCache<{ IMAGE_BUFFER_CACHE_ID }>>,
        Block: &SMemoryImageBlock<IMAGE_BUFFER_CACHE_ID>,
        cur_image_index: u32,
    ) {
        if !Block.base.is_cached {
            frame_delay_buffer_cleanup[cur_image_index as usize].push(SDelayedBufferCleanupItem {
                buffer: Block.base.buffer,
                mem: Block.base.buffer_mem.clone(),
                mapped_data: std::ptr::null_mut(),
            });
        } else {
            image_buffer_caches
                .get_mut(&Block.image_memory_bits)
                .unwrap()
                .free_mem_block(&Block.base, cur_image_index as usize);
        }
    }

    pub fn UploadStreamedBuffer<const FlushForRendering: bool, TName>(
        device: &ash::Device,
        non_coherent_mem_alignment: u64,
        StreamedBuffer: &mut SStreamMemory<TName>,
        cur_image_index: u32,
    ) where
        TName: Clone + StreamMemory + Default,
    {
        let mut RangeUpdateCount: usize = 0;
        if StreamedBuffer.is_used(cur_image_index as usize) {
            for i in 0..StreamedBuffer.get_used_count(cur_image_index as usize) {
                let (BuffersOfFrame, MemRanges) =
                    StreamedBuffer.get_buffers_and_ranges(cur_image_index as usize);
                let (BufferOfFrame, MemRange) =
                    (&mut BuffersOfFrame[i], &mut MemRanges[RangeUpdateCount]);
                RangeUpdateCount += 1;
                MemRange.s_type = vk::StructureType::MAPPED_MEMORY_RANGE;
                MemRange.memory = BufferOfFrame.get_device_mem_block().mem;
                MemRange.offset = *BufferOfFrame.get_offset_in_buffer() as u64;
                let AlignmentMod =
                    *BufferOfFrame.get_used_size() as vk::DeviceSize % non_coherent_mem_alignment;
                let mut AlignmentReq = non_coherent_mem_alignment - AlignmentMod;
                if AlignmentMod == 0 {
                    AlignmentReq = 0;
                }
                MemRange.size = *BufferOfFrame.get_used_size() as u64 + AlignmentReq;

                if MemRange.offset + MemRange.size > BufferOfFrame.get_device_mem_block().size {
                    MemRange.size = vk::WHOLE_SIZE;
                }

                *BufferOfFrame.get_used_size() = 0;
            }
            if RangeUpdateCount > 0 && FlushForRendering {
                unsafe {
                    device.flush_mapped_memory_ranges(
                        StreamedBuffer
                            .get_ranges(cur_image_index as usize)
                            .split_at(RangeUpdateCount)
                            .0,
                    );
                }
            }
        }
        StreamedBuffer.reset_frame(cur_image_index as usize);
    }

    pub fn PrepareStagingMemRangeImpl(
        &mut self,
        buffer_mem: &SDeviceMemoryBlock,
        heap_data: &SMemoryHeapQueueElement,
    ) {
        let mut UploadRange = vk::MappedMemoryRange::default();
        UploadRange.memory = buffer_mem.mem;
        UploadRange.offset = heap_data.offset_to_align as u64;

        let AlignmentMod =
            heap_data.allocation_size as vk::DeviceSize % self.limits.non_coherent_mem_alignment;
        let mut AlignmentReq = self.limits.non_coherent_mem_alignment - AlignmentMod;
        if AlignmentMod == 0 {
            AlignmentReq = 0;
        }
        UploadRange.size = (heap_data.allocation_size as u64 + AlignmentReq) as u64;

        if UploadRange.offset + UploadRange.size > buffer_mem.size {
            UploadRange.size = vk::WHOLE_SIZE;
        }

        self.non_flushed_staging_buffer_ranges.push(UploadRange);
    }

    pub fn PrepareStagingMemRange<const ID: usize>(&mut self, block: &mut SMemoryBlock<ID>) {
        self.PrepareStagingMemRangeImpl(&block.buffer_mem, &block.heap_data);
    }

    pub fn UploadAndFreeStagingMemBlock(
        &mut self,
        Block: &mut SMemoryBlock<STAGING_BUFFER_CACHE_ID>,
        cur_image_index: u32,
    ) {
        self.PrepareStagingMemRange(Block);
        if !Block.is_cached {
            self.frame_delayed_buffer_cleanups[cur_image_index as usize].push(
                SDelayedBufferCleanupItem {
                    buffer: Block.buffer,
                    mem: Block.buffer_mem.clone(),
                    mapped_data: Block.mapped_buffer,
                },
            );
        } else {
            self.staging_buffer_cache
                .free_mem_block(Block, cur_image_index as usize);
        }
    }

    pub fn UploadAndFreeStagingImageMemBlock(
        &mut self,
        Block: &mut SMemoryBlock<STAGING_BUFFER_IMAGE_CACHE_ID>,
        cur_image_index: u32,
    ) {
        self.PrepareStagingMemRange(Block);
        if !Block.is_cached {
            self.frame_delayed_buffer_cleanups[cur_image_index as usize].push(
                SDelayedBufferCleanupItem {
                    buffer: Block.buffer,
                    mem: Block.buffer_mem.clone(),
                    mapped_data: Block.mapped_buffer,
                },
            );
        } else {
            self.staging_buffer_cache_image
                .free_mem_block(Block, cur_image_index as usize);
        }
    }

    #[must_use]
    pub fn GetVertexBuffer(
        &mut self,
        ResBlock: &mut SMemoryBlock<VERTEX_BUFFER_CACHE_ID>,
        RequiredSize: vk::DeviceSize,
    ) -> bool {
        return self
            .mem
            .GetBufferBlockImpl::<{ VERTEX_BUFFER_CACHE_ID }, { 8 * 1024 * 1024 }, 3, false>(
                ResBlock,
                &mut self.vertex_buffer_cache,
                vk::BufferUsageFlags::TRANSFER_DST | vk::BufferUsageFlags::VERTEX_BUFFER,
                vk::MemoryPropertyFlags::DEVICE_LOCAL,
                std::ptr::null(),
                RequiredSize,
                16,
            );
    }

    pub fn FreeVertexMemBlock(
        frame_delayed_buffer_cleanup: &mut Vec<Vec<SDelayedBufferCleanupItem>>,
        vertex_buffer_cache: &mut SMemoryBlockCache<{ VERTEX_BUFFER_CACHE_ID }>,
        Block: &SMemoryBlock<VERTEX_BUFFER_CACHE_ID>,
        cur_image_index: u32,
    ) {
        if !Block.is_cached {
            frame_delayed_buffer_cleanup[cur_image_index as usize].push(
                SDelayedBufferCleanupItem {
                    buffer: Block.buffer,
                    mem: Block.buffer_mem.clone(),
                    mapped_data: std::ptr::null_mut(),
                },
            );
        } else {
            vertex_buffer_cache.free_mem_block(Block, cur_image_index as usize);
        }
    }

    pub fn DestroyTexture(
        frame_delay_buffer_cleanup: &mut Vec<Vec<SDelayedBufferCleanupItem>>,
        image_buffer_caches: &mut BTreeMap<u32, SMemoryBlockCache<{ IMAGE_BUFFER_CACHE_ID }>>,
        device: &ash::Device,
        Texture: &mut CTexture,
        cur_image_index: u32,
    ) {
        if Texture.img != vk::Image::null() {
            Self::FreeImageMemBlock(
                frame_delay_buffer_cleanup,
                image_buffer_caches,
                &mut Texture.img_mem,
                cur_image_index,
            );
            unsafe {
                device.destroy_image(Texture.img, None);
            }

            unsafe {
                device.destroy_image_view(Texture.img_view, None);
            }
        }

        if Texture.img_3d != vk::Image::null() {
            Self::FreeImageMemBlock(
                frame_delay_buffer_cleanup,
                image_buffer_caches,
                &mut Texture.img_3d_mem,
                cur_image_index,
            );
            unsafe {
                device.destroy_image(Texture.img_3d, None);
            }

            unsafe {
                device.destroy_image_view(Texture.img_3d_view, None);
            }
        }

        Self::DestroyTexturedStandardDescriptorSets(&device, Texture, 0);
        Self::DestroyTexturedStandardDescriptorSets(&device, Texture, 1);

        Self::DestroyTextured3DStandardDescriptorSets(&device, Texture);
    }

    pub fn DestroyTextTexture(
        frame_delay_buffer_cleanup: &mut Vec<Vec<SDelayedBufferCleanupItem>>,
        image_buffer_caches: &mut BTreeMap<u32, SMemoryBlockCache<{ IMAGE_BUFFER_CACHE_ID }>>,
        device: &ash::Device,
        Texture: &mut CTexture,
        TextureOutline: &mut CTexture,
        cur_image_index: u32,
    ) {
        if Texture.img != vk::Image::null() {
            Self::FreeImageMemBlock(
                frame_delay_buffer_cleanup,
                image_buffer_caches,
                &mut Texture.img_mem,
                cur_image_index,
            );
            unsafe {
                device.destroy_image(Texture.img, None);
            }

            unsafe {
                device.destroy_image_view(Texture.img_view, None);
            }
        }

        if TextureOutline.img != vk::Image::null() {
            Self::FreeImageMemBlock(
                frame_delay_buffer_cleanup,
                image_buffer_caches,
                &mut TextureOutline.img_mem,
                cur_image_index,
            );
            unsafe {
                device.destroy_image(TextureOutline.img, None);
            }

            unsafe {
                device.destroy_image_view(TextureOutline.img_view, None);
            }
        }

        Self::DestroyTextDescriptorSets(device, Texture, TextureOutline);
    }

    pub fn ShrinkUnusedCaches(&mut self) {
        let mut FreeedMemory: usize = 0;
        FreeedMemory += self.staging_buffer_cache.shrink(&mut self.device);
        FreeedMemory += self.staging_buffer_cache_image.shrink(&mut self.device);
        if FreeedMemory > 0 {
            unsafe {
                self.staging_memory_usage
                    .fetch_sub(FreeedMemory as u64, std::sync::atomic::Ordering::Relaxed);
            }
            if is_verbose(&*self.dbg) {
                // TODO dbg_msg("vulkan", "deallocated chunks of memory with size: %" PRIu64 " from all frames (staging buffer)", (usize)FreeedMemory);
            }
        }
        FreeedMemory = 0;
        FreeedMemory += self.vertex_buffer_cache.shrink(&mut self.device);
        if FreeedMemory > 0 {
            unsafe {
                self.buffer_memory_usage
                    .fetch_sub(FreeedMemory as u64, std::sync::atomic::Ordering::Relaxed);
            }
            if is_verbose(&*self.dbg) {
                // TODO dbg_msg("vulkan", "deallocated chunks of memory with size: %" PRIu64 " from all frames (buffer)", (usize)FreeedMemory);
            }
        }
        FreeedMemory = 0;
        for ImageBufferCache in &mut self.image_buffer_caches {
            FreeedMemory += ImageBufferCache.1.shrink(&mut self.device);
        }
        if FreeedMemory > 0 {
            unsafe {
                self.texture_memory_usage
                    .fetch_sub(FreeedMemory as u64, std::sync::atomic::Ordering::Relaxed);
            }
            if is_verbose(&*self.dbg) {
                // TODO dbg_msg("vulkan", "deallocated chunks of memory with size: %" PRIu64 " from all frames (texture)", (usize)FreeedMemory);
            }
        }
    }

    #[must_use]
    pub fn GetMemoryCommandBuffer(
        &mut self,
        pMemCommandBuffer: &mut *mut vk::CommandBuffer,
        cur_image_index: u32,
    ) -> bool {
        let MemCommandBuffer = &mut self.memory_command_buffers[cur_image_index as usize];
        if !self.used_memory_command_buffer[cur_image_index as usize] {
            self.used_memory_command_buffer[cur_image_index as usize] = true;

            if unsafe {
                self.device.reset_command_buffer(
                    *MemCommandBuffer,
                    vk::CommandBufferResetFlags::RELEASE_RESOURCES,
                )
            }
            .is_err()
            {
                self.error.lock().unwrap().SetError(
                    EGFXErrorType::RenderRecording,
                    localizable("Command buffer cannot be resetted anymore."),
                );
                return false;
            }

            let mut BeginInfo = vk::CommandBufferBeginInfo::default();
            BeginInfo.flags = vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT;
            unsafe {
                if self
                    .device
                    .begin_command_buffer(*MemCommandBuffer, &BeginInfo)
                    .is_err()
                {
                    self.error.lock().unwrap().SetError(
                        EGFXErrorType::RenderRecording,
                        localizable("Command buffer cannot be filled anymore."),
                    );
                    return false;
                }
            }
        }
        *pMemCommandBuffer = MemCommandBuffer;
        return true;
    }

    #[must_use]
    pub fn MemoryBarrier(
        &mut self,
        Buffer: vk::Buffer,
        Offset: vk::DeviceSize,
        Size: vk::DeviceSize,
        BufferAccessType: vk::AccessFlags,
        BeforeCommand: bool,
        cur_image_index: u32,
    ) -> bool {
        let mut pMemCommandBuffer: *mut vk::CommandBuffer = std::ptr::null_mut();
        if !self.GetMemoryCommandBuffer(&mut pMemCommandBuffer, cur_image_index) {
            return false;
        }
        let MemCommandBuffer = unsafe { &mut *pMemCommandBuffer };

        let mut Barrier = vk::BufferMemoryBarrier::default();
        Barrier.src_queue_family_index = vk::QUEUE_FAMILY_IGNORED;
        Barrier.dst_queue_family_index = vk::QUEUE_FAMILY_IGNORED;
        Barrier.buffer = Buffer;
        Barrier.offset = Offset;
        Barrier.size = Size;

        let mut SourceStage = vk::PipelineStageFlags::TOP_OF_PIPE;
        let mut DestinationStage = vk::PipelineStageFlags::TRANSFER;

        if BeforeCommand {
            Barrier.src_access_mask = BufferAccessType;
            Barrier.dst_access_mask = vk::AccessFlags::TRANSFER_WRITE;

            SourceStage = vk::PipelineStageFlags::VERTEX_INPUT;
            DestinationStage = vk::PipelineStageFlags::TRANSFER;
        } else {
            Barrier.src_access_mask = vk::AccessFlags::TRANSFER_WRITE;
            Barrier.dst_access_mask = BufferAccessType;

            SourceStage = vk::PipelineStageFlags::TRANSFER;
            DestinationStage = vk::PipelineStageFlags::VERTEX_INPUT;
        }

        unsafe {
            self.device.cmd_pipeline_barrier(
                *MemCommandBuffer,
                SourceStage,
                DestinationStage,
                vk::DependencyFlags::empty(),
                &[],
                &[Barrier],
                &[],
            );
        }

        return true;
    }

    /************************
     * TEXTURES
     ************************/

    #[must_use]
    pub fn BuildMipmaps(
        &mut self,
        Image: vk::Image,
        _ImageFormat: vk::Format,
        Width: usize,
        Height: usize,
        Depth: usize,
        MipMapLevelCount: usize,
        cur_image_index: u32,
    ) -> bool {
        let mut pMemCommandBuffer: *mut vk::CommandBuffer = std::ptr::null_mut();
        if !self.GetMemoryCommandBuffer(&mut pMemCommandBuffer, cur_image_index) {
            return false;
        }
        let MemCommandBuffer = unsafe { &mut *pMemCommandBuffer };

        let mut Barrier = vk::ImageMemoryBarrier::default();
        Barrier.image = Image;
        Barrier.src_queue_family_index = vk::QUEUE_FAMILY_IGNORED;
        Barrier.dst_queue_family_index = vk::QUEUE_FAMILY_IGNORED;
        Barrier.subresource_range.aspect_mask = vk::ImageAspectFlags::COLOR;
        Barrier.subresource_range.level_count = 1;
        Barrier.subresource_range.base_array_layer = 0;
        Barrier.subresource_range.layer_count = Depth as u32;

        let mut TmpMipWidth: i32 = Width as i32;
        let mut TmpMipHeight: i32 = Height as i32;

        for i in 1..MipMapLevelCount {
            Barrier.subresource_range.base_mip_level = (i - 1) as u32;
            Barrier.old_layout = vk::ImageLayout::TRANSFER_DST_OPTIMAL;
            Barrier.new_layout = vk::ImageLayout::TRANSFER_SRC_OPTIMAL;
            Barrier.src_access_mask = vk::AccessFlags::TRANSFER_WRITE;
            Barrier.dst_access_mask = vk::AccessFlags::TRANSFER_READ;

            unsafe {
                self.device.cmd_pipeline_barrier(
                    *MemCommandBuffer,
                    vk::PipelineStageFlags::TRANSFER,
                    vk::PipelineStageFlags::TRANSFER,
                    vk::DependencyFlags::empty(),
                    &[],
                    &[],
                    &[Barrier],
                );
            }

            let mut Blit = vk::ImageBlit::default();
            Blit.src_offsets[0] = vk::Offset3D::default();
            Blit.src_offsets[1] = vk::Offset3D {
                x: TmpMipWidth,
                y: TmpMipHeight,
                z: 1,
            };
            Blit.src_subresource.aspect_mask = vk::ImageAspectFlags::COLOR;
            Blit.src_subresource.mip_level = (i - 1) as u32;
            Blit.src_subresource.base_array_layer = 0;
            Blit.src_subresource.layer_count = Depth as u32;
            Blit.dst_offsets[0] = vk::Offset3D::default();
            Blit.dst_offsets[1] = vk::Offset3D {
                x: if TmpMipWidth > 1 { TmpMipWidth / 2 } else { 1 },
                y: if TmpMipHeight > 1 {
                    TmpMipHeight / 2
                } else {
                    1
                },
                z: 1,
            };
            Blit.dst_subresource.aspect_mask = vk::ImageAspectFlags::COLOR;
            Blit.dst_subresource.mip_level = i as u32;
            Blit.dst_subresource.base_array_layer = 0;
            Blit.dst_subresource.layer_count = Depth as u32;

            unsafe {
                self.device.cmd_blit_image(
                    *MemCommandBuffer,
                    Image,
                    vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                    Image,
                    vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                    &[Blit],
                    if self.allows_linear_blitting {
                        vk::Filter::LINEAR
                    } else {
                        vk::Filter::NEAREST
                    },
                );
            }

            Barrier.old_layout = vk::ImageLayout::TRANSFER_SRC_OPTIMAL;
            Barrier.new_layout = vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL;
            Barrier.src_access_mask = vk::AccessFlags::TRANSFER_READ;
            Barrier.dst_access_mask = vk::AccessFlags::SHADER_READ;

            unsafe {
                self.device.cmd_pipeline_barrier(
                    *MemCommandBuffer,
                    vk::PipelineStageFlags::TRANSFER,
                    vk::PipelineStageFlags::FRAGMENT_SHADER,
                    vk::DependencyFlags::empty(),
                    &[],
                    &[],
                    &[Barrier],
                );
            }

            if TmpMipWidth > 1 {
                TmpMipWidth /= 2;
            }
            if TmpMipHeight > 1 {
                TmpMipHeight /= 2;
            }
        }

        Barrier.subresource_range.base_mip_level = (MipMapLevelCount - 1) as u32;
        Barrier.old_layout = vk::ImageLayout::TRANSFER_DST_OPTIMAL;
        Barrier.new_layout = vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL;
        Barrier.src_access_mask = vk::AccessFlags::TRANSFER_WRITE;
        Barrier.dst_access_mask = vk::AccessFlags::SHADER_READ;

        unsafe {
            self.device.cmd_pipeline_barrier(
                *MemCommandBuffer,
                vk::PipelineStageFlags::TRANSFER,
                vk::PipelineStageFlags::FRAGMENT_SHADER,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                &[Barrier],
            );
        }

        return true;
    }

    #[must_use]
    pub fn CreateTextureImage(
        &mut self,
        _ImageIndex: usize,
        NewImage: &mut vk::Image,
        NewImgMem: &mut SMemoryImageBlock<IMAGE_BUFFER_CACHE_ID>,
        upload_data: &'static mut [u8],
        Format: vk::Format,
        width: usize,
        height: usize,
        depth: usize,
        pixel_size: usize,
        MipMapLevelCount: usize,
        cur_image_index: u32,
    ) -> bool {
        let image_size = width * height * depth * pixel_size;

        let allocator_dummy = self.mem_allocator.clone();
        let mut allocator_res = allocator_dummy.lock().unwrap();
        let mut allocator = allocator_res.as_mut().unwrap();
        let buffer_block_res = allocator.get_mem_block_image(upload_data.as_ptr() as *mut c_void);

        let staging_buffer = buffer_block_res.unwrap();

        let ImgFormat = Format;

        if !self.CreateImage(
            width as u32,
            height as u32,
            depth as u32,
            MipMapLevelCount,
            ImgFormat,
            vk::ImageTiling::OPTIMAL,
            NewImage,
            NewImgMem,
        ) {
            return false;
        }

        if !self.ImageBarrier(
            *NewImage,
            0,
            MipMapLevelCount,
            0,
            depth,
            ImgFormat,
            vk::ImageLayout::UNDEFINED,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            cur_image_index,
        ) {
            return false;
        }
        if !self.CopyBufferToImage(
            staging_buffer.buffer,
            staging_buffer.heap_data.offset_to_align as u64,
            *NewImage,
            0,
            0,
            width as u32,
            height as u32,
            depth,
            cur_image_index,
        ) {
            return false;
        }

        allocator.upload_and_free_mem(upload_data, cur_image_index, |block, queue_el| {
            self.PrepareStagingMemRangeImpl(block, queue_el);
        });

        if MipMapLevelCount > 1 {
            if !self.BuildMipmaps(
                *NewImage,
                ImgFormat,
                width,
                height,
                depth,
                MipMapLevelCount,
                cur_image_index,
            ) {
                return false;
            }
        } else {
            if !self.ImageBarrier(
                *NewImage,
                0,
                1,
                0,
                depth,
                ImgFormat,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                cur_image_index,
            ) {
                return false;
            }
        }

        return true;
    }

    pub fn CreateTextureImageView(
        &mut self,
        TexImage: vk::Image,
        ImgFormat: vk::Format,
        ViewType: vk::ImageViewType,
        Depth: usize,
        MipMapLevelCount: usize,
    ) -> vk::ImageView {
        return self.CreateImageView(TexImage, ImgFormat, ViewType, Depth, MipMapLevelCount);
    }

    #[must_use]
    pub fn CreateTextureSamplersImpl(
        device: &ash::Device,
        max_sampler_anisotropy: u32,
        global_texture_lod_bias: i32,
        CreatedSampler: &mut vk::Sampler,
        AddrModeU: vk::SamplerAddressMode,
        AddrModeV: vk::SamplerAddressMode,
        AddrModeW: vk::SamplerAddressMode,
    ) -> bool {
        let mut SamplerInfo = vk::SamplerCreateInfo::default();
        SamplerInfo.mag_filter = vk::Filter::LINEAR;
        SamplerInfo.min_filter = vk::Filter::LINEAR;
        SamplerInfo.address_mode_u = AddrModeU;
        SamplerInfo.address_mode_v = AddrModeV;
        SamplerInfo.address_mode_w = AddrModeW;
        SamplerInfo.anisotropy_enable = vk::FALSE;
        SamplerInfo.max_anisotropy = max_sampler_anisotropy as f32;
        SamplerInfo.border_color = vk::BorderColor::INT_OPAQUE_BLACK;
        SamplerInfo.unnormalized_coordinates = vk::FALSE;
        SamplerInfo.compare_enable = vk::FALSE;
        SamplerInfo.compare_op = vk::CompareOp::ALWAYS;
        SamplerInfo.mipmap_mode = vk::SamplerMipmapMode::LINEAR;
        SamplerInfo.mip_lod_bias = global_texture_lod_bias as f32 / 1000.0;
        SamplerInfo.min_lod = -1000.0;
        SamplerInfo.max_lod = 1000.0;

        let res = unsafe { device.create_sampler(&SamplerInfo, None) };
        if let Err(_) = res {
            // TODO dbg_msg("vulkan", "failed to create texture sampler!");
            return false;
        }
        *CreatedSampler = res.unwrap();
        return true;
    }

    pub fn CreateImageView(
        &mut self,
        Image: vk::Image,
        Format: vk::Format,
        ViewType: vk::ImageViewType,
        Depth: usize,
        MipMapLevelCount: usize,
    ) -> vk::ImageView {
        let mut ViewCreateInfo = vk::ImageViewCreateInfo::default();
        ViewCreateInfo.image = Image;
        ViewCreateInfo.view_type = ViewType;
        ViewCreateInfo.format = Format;
        ViewCreateInfo.subresource_range.aspect_mask = vk::ImageAspectFlags::COLOR;
        ViewCreateInfo.subresource_range.base_mip_level = 0;
        ViewCreateInfo.subresource_range.level_count = MipMapLevelCount as u32;
        ViewCreateInfo.subresource_range.base_array_layer = 0;
        ViewCreateInfo.subresource_range.layer_count = Depth as u32;

        let mut ImageView = vk::ImageView::default();
        let res = unsafe { self.device.create_image_view(&ViewCreateInfo, None) };
        if let Err(_) = res {
            return vk::ImageView::null();
        }
        ImageView = res.unwrap();
        return ImageView;
    }

    pub fn GetMaxSampleCount(limits: &Limits) -> vk::SampleCountFlags {
        if !(limits.max_multi_sample & vk::SampleCountFlags::TYPE_64).is_empty() {
            return vk::SampleCountFlags::TYPE_64;
        } else if !(limits.max_multi_sample & vk::SampleCountFlags::TYPE_32).is_empty() {
            return vk::SampleCountFlags::TYPE_32;
        } else if !(limits.max_multi_sample & vk::SampleCountFlags::TYPE_16).is_empty() {
            return vk::SampleCountFlags::TYPE_16;
        } else if !(limits.max_multi_sample & vk::SampleCountFlags::TYPE_8).is_empty() {
            return vk::SampleCountFlags::TYPE_8;
        } else if !(limits.max_multi_sample & vk::SampleCountFlags::TYPE_4).is_empty() {
            return vk::SampleCountFlags::TYPE_4;
        } else if !(limits.max_multi_sample & vk::SampleCountFlags::TYPE_2).is_empty() {
            return vk::SampleCountFlags::TYPE_2;
        }

        return vk::SampleCountFlags::TYPE_1;
    }

    pub fn GetSampleCount(limits: &Limits) -> vk::SampleCountFlags {
        let MaxSampleCount = Self::GetMaxSampleCount(limits);
        if limits.multi_sampling_count >= 64 && MaxSampleCount >= vk::SampleCountFlags::TYPE_64 {
            return vk::SampleCountFlags::TYPE_64;
        } else if limits.multi_sampling_count >= 32
            && MaxSampleCount >= vk::SampleCountFlags::TYPE_32
        {
            return vk::SampleCountFlags::TYPE_32;
        } else if limits.multi_sampling_count >= 16
            && MaxSampleCount >= vk::SampleCountFlags::TYPE_16
        {
            return vk::SampleCountFlags::TYPE_16;
        } else if limits.multi_sampling_count >= 8 && MaxSampleCount >= vk::SampleCountFlags::TYPE_8
        {
            return vk::SampleCountFlags::TYPE_8;
        } else if limits.multi_sampling_count >= 4 && MaxSampleCount >= vk::SampleCountFlags::TYPE_4
        {
            return vk::SampleCountFlags::TYPE_4;
        } else if limits.multi_sampling_count >= 2 && MaxSampleCount >= vk::SampleCountFlags::TYPE_2
        {
            return vk::SampleCountFlags::TYPE_2;
        }

        return vk::SampleCountFlags::TYPE_1;
    }

    #[must_use]
    pub fn CreateImageEx(
        &mut self,
        Width: u32,
        Height: u32,
        Depth: u32,
        MipMapLevelCount: usize,
        Format: vk::Format,
        Tiling: vk::ImageTiling,
        Image: &mut vk::Image,
        ImageMemory: &mut SMemoryImageBlock<IMAGE_BUFFER_CACHE_ID>,
        ImageUsage: vk::ImageUsageFlags,
    ) -> bool {
        let mut ImageInfo = vk::ImageCreateInfo::default();
        ImageInfo.image_type = vk::ImageType::TYPE_2D;
        ImageInfo.extent.width = Width;
        ImageInfo.extent.height = Height;
        ImageInfo.extent.depth = 1;
        ImageInfo.mip_levels = MipMapLevelCount as u32;
        ImageInfo.array_layers = Depth;
        ImageInfo.format = Format;
        ImageInfo.tiling = Tiling;
        ImageInfo.initial_layout = vk::ImageLayout::UNDEFINED;
        ImageInfo.usage = ImageUsage;
        ImageInfo.samples = if (ImageUsage & vk::ImageUsageFlags::COLOR_ATTACHMENT)
            == vk::ImageUsageFlags::empty()
        {
            vk::SampleCountFlags::TYPE_1
        } else {
            Self::GetSampleCount(&self.limits)
        };
        ImageInfo.sharing_mode = vk::SharingMode::EXCLUSIVE;

        let res = unsafe { self.device.create_image(&ImageInfo, None) };
        if res.is_err() {
            // TODO dbg_msg("vulkan", "failed to create image!");
        }
        *Image = res.unwrap();

        let mem_requirements = unsafe { self.device.get_image_memory_requirements(*Image) };

        if !self.GetImageMemory(
            ImageMemory,
            mem_requirements.size,
            mem_requirements.alignment,
            mem_requirements.memory_type_bits,
        ) {
            return false;
        }

        unsafe {
            self.device.bind_image_memory(
                *Image,
                ImageMemory.base.buffer_mem.mem,
                ImageMemory.base.heap_data.offset_to_align as u64,
            );
        }

        return true;
    }

    #[must_use]
    pub fn CreateImage(
        &mut self,
        Width: u32,
        Height: u32,
        Depth: u32,
        MipMapLevelCount: usize,
        Format: vk::Format,
        Tiling: vk::ImageTiling,
        Image: &mut vk::Image,
        ImageMemory: &mut SMemoryImageBlock<IMAGE_BUFFER_CACHE_ID>,
    ) -> bool {
        return self.CreateImageEx(
            Width,
            Height,
            Depth,
            MipMapLevelCount,
            Format,
            Tiling,
            Image,
            ImageMemory,
            vk::ImageUsageFlags::TRANSFER_SRC
                | vk::ImageUsageFlags::TRANSFER_DST
                | vk::ImageUsageFlags::SAMPLED,
        );
    }

    #[must_use]
    pub fn ImageBarrier(
        &mut self,
        Image: vk::Image,
        MipMapBase: usize,
        MipMapCount: usize,
        LayerBase: usize,
        LayerCount: usize,
        _Format: vk::Format,
        OldLayout: vk::ImageLayout,
        NewLayout: vk::ImageLayout,
        cur_image_index: u32,
    ) -> bool {
        let mut pMemCommandBuffer: *mut vk::CommandBuffer = std::ptr::null_mut();
        if !self.GetMemoryCommandBuffer(&mut pMemCommandBuffer, cur_image_index) {
            return false;
        }
        let MemCommandBuffer = unsafe { &mut *pMemCommandBuffer };

        let mut Barrier = vk::ImageMemoryBarrier::default();
        Barrier.old_layout = OldLayout;
        Barrier.new_layout = NewLayout;
        Barrier.src_queue_family_index = vk::QUEUE_FAMILY_IGNORED;
        Barrier.dst_queue_family_index = vk::QUEUE_FAMILY_IGNORED;
        Barrier.image = Image;
        Barrier.subresource_range.aspect_mask = vk::ImageAspectFlags::COLOR;
        Barrier.subresource_range.base_mip_level = MipMapBase as u32;
        Barrier.subresource_range.level_count = MipMapCount as u32;
        Barrier.subresource_range.base_array_layer = LayerBase as u32;
        Barrier.subresource_range.layer_count = LayerCount as u32;

        let mut SourceStage = vk::PipelineStageFlags::TOP_OF_PIPE;
        let mut DestinationStage = vk::PipelineStageFlags::TRANSFER;

        if OldLayout == vk::ImageLayout::UNDEFINED
            && NewLayout == vk::ImageLayout::TRANSFER_DST_OPTIMAL
        {
            Barrier.src_access_mask = vk::AccessFlags::empty();
            Barrier.dst_access_mask = vk::AccessFlags::TRANSFER_WRITE;

            SourceStage = vk::PipelineStageFlags::TOP_OF_PIPE;
            DestinationStage = vk::PipelineStageFlags::TRANSFER;
        } else if OldLayout == vk::ImageLayout::TRANSFER_DST_OPTIMAL
            && NewLayout == vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL
        {
            Barrier.src_access_mask = vk::AccessFlags::TRANSFER_WRITE;
            Barrier.dst_access_mask = vk::AccessFlags::SHADER_READ;

            SourceStage = vk::PipelineStageFlags::TRANSFER;
            DestinationStage = vk::PipelineStageFlags::FRAGMENT_SHADER;
        } else if OldLayout == vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL
            && NewLayout == vk::ImageLayout::TRANSFER_DST_OPTIMAL
        {
            Barrier.src_access_mask = vk::AccessFlags::SHADER_READ;
            Barrier.dst_access_mask = vk::AccessFlags::TRANSFER_WRITE;

            SourceStage = vk::PipelineStageFlags::FRAGMENT_SHADER;
            DestinationStage = vk::PipelineStageFlags::TRANSFER;
        } else if OldLayout == vk::ImageLayout::TRANSFER_SRC_OPTIMAL
            && NewLayout == vk::ImageLayout::PRESENT_SRC_KHR
        {
            Barrier.src_access_mask = vk::AccessFlags::TRANSFER_READ;
            Barrier.dst_access_mask = vk::AccessFlags::MEMORY_READ;

            SourceStage = vk::PipelineStageFlags::TRANSFER;
            DestinationStage = vk::PipelineStageFlags::BOTTOM_OF_PIPE;
        } else if OldLayout == vk::ImageLayout::PRESENT_SRC_KHR
            && NewLayout == vk::ImageLayout::TRANSFER_SRC_OPTIMAL
        {
            Barrier.src_access_mask = vk::AccessFlags::MEMORY_READ;
            Barrier.dst_access_mask = vk::AccessFlags::TRANSFER_READ;

            SourceStage = vk::PipelineStageFlags::BOTTOM_OF_PIPE;
            DestinationStage = vk::PipelineStageFlags::TRANSFER;
        } else if OldLayout == vk::ImageLayout::UNDEFINED && NewLayout == vk::ImageLayout::GENERAL {
            Barrier.src_access_mask = vk::AccessFlags::empty();
            Barrier.dst_access_mask = vk::AccessFlags::MEMORY_READ;

            SourceStage = vk::PipelineStageFlags::TOP_OF_PIPE;
            DestinationStage = vk::PipelineStageFlags::TRANSFER;
        } else if OldLayout == vk::ImageLayout::GENERAL
            && NewLayout == vk::ImageLayout::TRANSFER_DST_OPTIMAL
        {
            Barrier.src_access_mask = vk::AccessFlags::MEMORY_READ;
            Barrier.dst_access_mask = vk::AccessFlags::TRANSFER_WRITE;

            SourceStage = vk::PipelineStageFlags::TRANSFER;
            DestinationStage = vk::PipelineStageFlags::TRANSFER;
        } else if OldLayout == vk::ImageLayout::TRANSFER_DST_OPTIMAL
            && NewLayout == vk::ImageLayout::GENERAL
        {
            Barrier.src_access_mask = vk::AccessFlags::TRANSFER_WRITE;
            Barrier.dst_access_mask = vk::AccessFlags::MEMORY_READ;

            SourceStage = vk::PipelineStageFlags::TRANSFER;
            DestinationStage = vk::PipelineStageFlags::TRANSFER;
        } else {
            // TODO dbg_msg("vulkan", "unsupported layout transition!");
        }

        unsafe {
            self.device.cmd_pipeline_barrier(
                *MemCommandBuffer,
                SourceStage,
                DestinationStage,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                &[Barrier],
            );
        }

        return true;
    }

    #[must_use]
    pub fn CopyBufferToImage(
        &mut self,
        Buffer: vk::Buffer,
        BufferOffset: vk::DeviceSize,
        Image: vk::Image,
        X: i32,
        Y: i32,
        Width: u32,
        Height: u32,
        Depth: usize,
        cur_image_index: u32,
    ) -> bool {
        let mut command_buffer_ptr: *mut vk::CommandBuffer = std::ptr::null_mut();
        if !self.GetMemoryCommandBuffer(&mut command_buffer_ptr, cur_image_index) {
            return false;
        }
        let CommandBuffer = unsafe { &mut *command_buffer_ptr };

        let mut Region = vk::BufferImageCopy::default();
        Region.buffer_offset = BufferOffset;
        Region.buffer_row_length = 0;
        Region.buffer_image_height = 0;
        Region.image_subresource.aspect_mask = vk::ImageAspectFlags::COLOR;
        Region.image_subresource.mip_level = 0;
        Region.image_subresource.base_array_layer = 0;
        Region.image_subresource.layer_count = Depth as u32;
        Region.image_offset = vk::Offset3D { x: X, y: Y, z: 0 };
        Region.image_extent = vk::Extent3D {
            width: Width,
            height: Height,
            depth: 1,
        };

        unsafe {
            self.device.cmd_copy_buffer_to_image(
                *CommandBuffer,
                Buffer,
                Image,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                &[Region],
            );
        }

        return true;
    }

    /************************
     * STREAM BUFFERS SETUP
     ************************/

    #[must_use]
    pub fn create_stream_buffer_unallocated<
        'a,
        TStreamMemName: Clone,
        TInstanceTypeName,
        const InstanceTypeCount: usize,
        const BufferCreateCount: usize,
        const UsesCurrentCountOffset: bool,
    >(
        mem: &Memory,
        device: &ash::Device,
        pBufferMem: &mut *mut TStreamMemName,
        NewMemFunc: &'a mut dyn FnMut(&mut TStreamMemName, vk::Buffer, vk::DeviceSize) -> bool,
        StreamUniformBuffer: &mut SStreamMemory<TStreamMemName>,
        Usage: vk::BufferUsageFlags,
        DataSize: usize,
        cur_image_index: u32,

        Buffer: &mut vk::Buffer,
        BufferMem: &mut SDeviceMemoryBlock,
        Offset: &mut usize,

        pMem: &mut *mut u8,
    ) -> bool
    where
        TStreamMemName: Default + StreamMemory,
    {
        let mut it: usize = 0;
        if UsesCurrentCountOffset {
            it = StreamUniformBuffer.get_used_count(cur_image_index as usize);
        }
        while it
            < StreamUniformBuffer
                .get_buffers(cur_image_index as usize)
                .len()
        {
            let mut BufferOfFrame =
                &mut StreamUniformBuffer.get_buffers(cur_image_index as usize)[it];
            if *BufferOfFrame.get_size() >= DataSize + *BufferOfFrame.get_used_size() {
                if *BufferOfFrame.get_used_size() == 0 {
                    StreamUniformBuffer.increase_used_count(cur_image_index as usize);
                    BufferOfFrame =
                        &mut StreamUniformBuffer.get_buffers(cur_image_index as usize)[it];
                }
                *Buffer = *BufferOfFrame.get_buffer();
                *BufferMem = BufferOfFrame.get_device_mem_block().clone();
                *Offset = *BufferOfFrame.get_used_size();
                *BufferOfFrame.get_used_size() += DataSize;
                *pMem = *BufferOfFrame.get_mapped_buffer_data() as *mut u8;
                *pBufferMem = BufferOfFrame;
                break;
            }
            it += 1;
        }

        if BufferMem.mem == vk::DeviceMemory::null() {
            // create memory
            let mut StreamBuffer = vk::Buffer::null();
            let mut StreamBufferMemory = SDeviceMemoryBlock::default();
            let NewBufferSingleSize =
                (std::mem::size_of::<TInstanceTypeName>() * InstanceTypeCount) as vk::DeviceSize;
            let NewBufferSize = (NewBufferSingleSize * BufferCreateCount as u64) as vk::DeviceSize;
            if !mem.CreateBuffer(
                NewBufferSize,
                EMemoryBlockUsage::Stream,
                Usage,
                vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_CACHED,
                &mut StreamBuffer,
                &mut StreamBufferMemory,
            ) {
                return false;
            }

            let mut pMappedData: *mut c_void = std::ptr::null_mut();
            pMappedData = unsafe {
                device.map_memory(
                    StreamBufferMemory.mem,
                    0,
                    vk::WHOLE_SIZE,
                    vk::MemoryMapFlags::empty(),
                )
            }
            .unwrap();

            let NewBufferIndex: usize = StreamUniformBuffer
                .get_buffers(cur_image_index as usize)
                .len();
            for i in 0..BufferCreateCount {
                StreamUniformBuffer
                    .get_buffers(cur_image_index as usize)
                    .push(TStreamMemName::new(
                        StreamBuffer,
                        StreamBufferMemory.clone(),
                        NewBufferSingleSize as usize * i,
                        NewBufferSingleSize as usize,
                        0,
                        unsafe {
                            (pMappedData as *mut u8)
                                .offset((NewBufferSingleSize as isize * i as isize) as isize)
                        } as *mut c_void,
                    ));
                StreamUniformBuffer
                    .get_ranges(cur_image_index as usize)
                    .push(Default::default());
                if !NewMemFunc(
                    StreamUniformBuffer
                        .get_buffers(cur_image_index as usize)
                        .last_mut()
                        .unwrap(),
                    StreamBuffer,
                    NewBufferSingleSize * i as u64,
                ) {
                    return false;
                }
            }
            let NewStreamBuffer =
                &mut StreamUniformBuffer.get_buffers(cur_image_index as usize)[NewBufferIndex];

            *Buffer = StreamBuffer;
            *BufferMem = StreamBufferMemory;

            *pBufferMem = NewStreamBuffer;
            *pMem = *NewStreamBuffer.get_mapped_buffer_data() as *mut u8;
            *Offset = *NewStreamBuffer.get_offset_in_buffer();
            *NewStreamBuffer.get_used_size() += DataSize;

            StreamUniformBuffer.increase_used_count(cur_image_index as usize);
        }
        return true;
    }

    // returns true, if the stream memory was just allocated
    #[must_use]
    pub fn CreateStreamBuffer<
        'a,
        TStreamMemName: Clone,
        TInstanceTypeName,
        const InstanceTypeCount: usize,
        const BufferCreateCount: usize,
        const UsesCurrentCountOffset: bool,
    >(
        mem: &Memory,
        device: &ash::Device,
        pBufferMem: &mut *mut TStreamMemName,
        NewMemFunc: &'a mut dyn FnMut(&mut TStreamMemName, vk::Buffer, vk::DeviceSize) -> bool,
        StreamUniformBuffer: &mut SStreamMemory<TStreamMemName>,
        Usage: vk::BufferUsageFlags,
        NewBuffer: &mut vk::Buffer,
        NewBufferMem: &mut SDeviceMemoryBlock,
        BufferOffset: &mut usize,
        pData: *const c_void,
        DataSize: usize,
        cur_image_index: u32,
    ) -> bool
    where
        TStreamMemName: Default + StreamMemory,
    {
        let mut Buffer = vk::Buffer::null();
        let mut BufferMem = SDeviceMemoryBlock::default();
        let mut Offset: usize = 0;

        let mut pMem: *mut u8 = std::ptr::null_mut();

        Self::create_stream_buffer_unallocated::<
            TStreamMemName,
            TInstanceTypeName,
            InstanceTypeCount,
            BufferCreateCount,
            UsesCurrentCountOffset,
        >(
            mem,
            device,
            pBufferMem,
            NewMemFunc,
            StreamUniformBuffer,
            Usage,
            DataSize,
            cur_image_index,
            &mut Buffer,
            &mut BufferMem,
            &mut Offset,
            &mut pMem,
        );

        unsafe {
            libc::memcpy(
                pMem.offset(Offset as isize) as *mut c_void,
                pData,
                DataSize as usize,
            );
        }

        *NewBuffer = Buffer;
        *NewBufferMem = BufferMem;
        *BufferOffset = Offset;

        return true;
    }

    #[must_use]
    pub fn GetUniformBufferObjectImpl<
        TName,
        const InstanceMaxParticleCount: usize,
        const MaxInstances: usize,
    >(
        &mut self,
        RenderThreadIndex: usize,
        RequiresSharedStagesDescriptor: bool,
        DescrSet: &mut SDeviceDescriptorSet,
        pData: *const c_void,
        DataSize: usize,
        cur_image_index: u32,
    ) -> bool {
        let mut NewBuffer = vk::Buffer::null();
        let mut NewBufferMem = SDeviceMemoryBlock::default();
        let mut BufferOffset = usize::default();
        let mut pMem: *mut SFrameUniformBuffers = std::ptr::null_mut();
        let mem = &self.mem;
        let device = &self.device;
        let error = &self.error;
        let pools = &mut self.uniform_buffer_descr_pools[RenderThreadIndex];
        let sprite_descr_layout = &self.sprite_multi_uniform_descriptor_set_layout;
        let quad_descr_layout = &self.quad_uniform_descriptor_set_layout;
        let StreamUniformBuffer = &mut self.streamed_uniform_buffers[RenderThreadIndex];
        let mut new_mem_func = move |Mem: &mut SFrameUniformBuffers,
                                     Buffer: vk::Buffer,
                                     MemOffset: vk::DeviceSize|
              -> bool {
            if !Self::CreateUniformDescriptorSets(
                error,
                device,
                pools,
                sprite_descr_layout,
                unsafe { Mem.uniform_sets.as_mut_ptr() },
                1,
                Buffer,
                InstanceMaxParticleCount * std::mem::size_of::<TName>(),
                MemOffset,
            ) {
                return false;
            }
            if !Self::CreateUniformDescriptorSets(
                error,
                device,
                pools,
                quad_descr_layout,
                unsafe { &mut (*(Mem as *mut _ as *mut SFrameUniformBuffers)).uniform_sets[1] },
                1,
                Buffer,
                InstanceMaxParticleCount * std::mem::size_of::<TName>(),
                MemOffset,
            ) {
                return false;
            }
            return true;
        };
        if !Self::CreateStreamBuffer::<
            SFrameUniformBuffers,
            TName,
            InstanceMaxParticleCount,
            MaxInstances,
            true,
        >(
            mem,
            device,
            &mut pMem,
            &mut new_mem_func,
            StreamUniformBuffer,
            vk::BufferUsageFlags::UNIFORM_BUFFER,
            &mut NewBuffer,
            &mut NewBufferMem,
            &mut BufferOffset,
            pData,
            DataSize,
            cur_image_index,
        ) {
            return false;
        }

        *DescrSet = unsafe { &mut *pMem }.uniform_sets
            [if RequiresSharedStagesDescriptor { 1 } else { 0 }]
        .clone();
        return true;
    }

    #[must_use]
    pub fn CreateIndexBuffer(
        &mut self,
        pData: *const c_void,
        DataSize: usize,
        Buffer: &mut vk::Buffer,
        Memory: &mut SDeviceMemoryBlock,
        cur_image_index: u32,
    ) -> bool {
        let BufferDataSize = DataSize as vk::DeviceSize;

        let mut StagingBuffer = SMemoryBlock::<STAGING_BUFFER_CACHE_ID>::default();
        if !self.get_staging_buffer(&mut StagingBuffer, pData, DataSize as u64) {
            return false;
        }

        let mut VertexBufferMemory = SDeviceMemoryBlock::default();
        let mut VertexBuffer = vk::Buffer::null();
        if !self.mem.CreateBuffer(
            BufferDataSize,
            EMemoryBlockUsage::Buffer,
            vk::BufferUsageFlags::TRANSFER_DST | vk::BufferUsageFlags::INDEX_BUFFER,
            vk::MemoryPropertyFlags::DEVICE_LOCAL,
            &mut VertexBuffer,
            &mut VertexBufferMemory,
        ) {
            return false;
        }

        if !self.MemoryBarrier(
            VertexBuffer,
            0,
            BufferDataSize,
            vk::AccessFlags::INDEX_READ,
            true,
            cur_image_index,
        ) {
            return false;
        }
        if !self.CopyBuffer(
            StagingBuffer.buffer,
            VertexBuffer,
            StagingBuffer.heap_data.offset_to_align as u64,
            0,
            BufferDataSize,
            cur_image_index,
        ) {
            return false;
        }
        if !self.MemoryBarrier(
            VertexBuffer,
            0,
            BufferDataSize,
            vk::AccessFlags::INDEX_READ,
            false,
            cur_image_index,
        ) {
            return false;
        }

        self.UploadAndFreeStagingMemBlock(&mut StagingBuffer, cur_image_index);

        *Buffer = VertexBuffer;
        *Memory = VertexBufferMemory;
        return true;
    }

    pub fn DestroyIndexBuffer(&mut self, Buffer: &mut vk::Buffer, Memory: &mut SDeviceMemoryBlock) {
        self.mem.CleanBufferPair(0, Buffer, Memory);
    }

    /************************
     * BUFFERS
     ************************/
    #[must_use]
    pub fn CreateStreamVertexBuffer(
        &mut self,
        NewBuffer: &mut vk::Buffer,
        NewBufferMem: &mut SDeviceMemoryBlock,
        BufferOffset: &mut usize,
        pData: &mut *mut u8,
        DataSize: usize,
        cur_image_index: u32,
    ) -> bool {
        let mut pStreamBuffer: *mut SFrameBuffers = std::ptr::null_mut();
        return Self::create_stream_buffer_unallocated::<
            SFrameBuffers,
            GL_SVertexTex3DStream,
            { StreamDataMax::MaxVertices as usize * 2 },
            1,
            false,
        >(
            &self.mem,
            &self.device,
            &mut pStreamBuffer,
            &mut |_: &mut SFrameBuffers, _: vk::Buffer, _: vk::DeviceSize| -> bool {
                return true;
            },
            &mut self.streamed_vertex_buffer,
            vk::BufferUsageFlags::VERTEX_BUFFER,
            DataSize,
            cur_image_index,
            NewBuffer,
            NewBufferMem,
            BufferOffset,
            pData,
        );
    }

    #[must_use]
    pub fn update_stream_vertex_buffer(&mut self, DataSize: usize, cur_image_index: u32) {
        if !self
            .streamed_vertex_buffer
            .get_buffers(cur_image_index as usize)
            .is_empty()
        {
            let mut cur_buffer = self
                .streamed_vertex_buffer
                .get_current_buffer(cur_image_index as usize);

            cur_buffer.used_size += DataSize;
        }
    }

    #[must_use]
    pub fn CreateBufferObject(
        &mut self,
        BufferIndex: usize,
        pUploadData: &'static mut [u8],
        BufferDataSize: vk::DeviceSize,
        cur_image_index: u32,
    ) -> bool {
        while BufferIndex >= self.buffer_objects.len() {
            self.buffer_objects.resize(
                (self.buffer_objects.len() * 2) + 1,
                SBufferObjectFrame::default(),
            );
        }

        let mut VertexBuffer = vk::Buffer::null();
        let mut BufferOffset: usize = 0;
        let tmp_allocator = self.mem_allocator.clone();
        let mut mem_allocator = tmp_allocator.lock().unwrap();
        let allocator = mem_allocator.as_mut().unwrap();
        let mut staging_buffer = allocator
            .get_mem_block(pUploadData.as_ptr() as *mut c_void)
            .unwrap();

        let mut Mem = SMemoryBlock::<VERTEX_BUFFER_CACHE_ID>::default();
        if !self.GetVertexBuffer(&mut Mem, BufferDataSize) {
            return false;
        }

        let buffer_object = &mut self.buffer_objects[BufferIndex];
        buffer_object.buffer_object.mem = Mem.clone();
        VertexBuffer = Mem.buffer;
        BufferOffset = Mem.heap_data.offset_to_align;

        if !self.MemoryBarrier(
            VertexBuffer,
            Mem.heap_data.offset_to_align as u64,
            BufferDataSize,
            vk::AccessFlags::VERTEX_ATTRIBUTE_READ,
            true,
            cur_image_index,
        ) {
            return false;
        }
        if !self.CopyBuffer(
            staging_buffer.buffer,
            VertexBuffer,
            staging_buffer.heap_data.offset_to_align as u64,
            Mem.heap_data.offset_to_align as u64,
            BufferDataSize,
            cur_image_index,
        ) {
            return false;
        }
        if !self.MemoryBarrier(
            VertexBuffer,
            Mem.heap_data.offset_to_align as u64,
            BufferDataSize,
            vk::AccessFlags::VERTEX_ATTRIBUTE_READ,
            false,
            cur_image_index,
        ) {
            return false;
        }
        allocator.upload_and_free_mem(pUploadData, cur_image_index, |block, queue_el| {
            self.PrepareStagingMemRangeImpl(block, queue_el);
        });

        let buffer_object = &mut self.buffer_objects[BufferIndex];
        buffer_object.cur_buffer = VertexBuffer;
        buffer_object.cur_buffer_offset = BufferOffset;

        return true;
    }

    pub fn DeleteBufferObject(&mut self, BufferIndex: usize, cur_image_index: u32) {
        let mut DeleteObj: SBufferObjectFrame = Default::default();
        std::mem::swap(&mut DeleteObj, &mut self.buffer_objects[BufferIndex]);
        Self::FreeVertexMemBlock(
            &mut self.frame_delayed_buffer_cleanups,
            &mut self.vertex_buffer_cache,
            &DeleteObj.buffer_object.mem,
            cur_image_index,
        );
    }

    #[must_use]
    pub fn CopyBuffer(
        &mut self,
        SrcBuffer: vk::Buffer,
        DstBuffer: vk::Buffer,
        SrcOffset: vk::DeviceSize,
        DstOffset: vk::DeviceSize,
        CopySize: vk::DeviceSize,
        cur_image_index: u32,
    ) -> bool {
        let mut command_buffer_ptr: *mut vk::CommandBuffer = std::ptr::null_mut();
        if !self.GetMemoryCommandBuffer(&mut command_buffer_ptr, cur_image_index) {
            return false;
        }
        let CommandBuffer = unsafe { &mut *command_buffer_ptr };
        let mut CopyRegion = vk::BufferCopy::default();
        CopyRegion.src_offset = SrcOffset;
        CopyRegion.dst_offset = DstOffset;
        CopyRegion.size = CopySize;
        unsafe {
            self.device
                .cmd_copy_buffer(*CommandBuffer, SrcBuffer, DstBuffer, &[CopyRegion]);
        }

        return true;
    }

    #[must_use]
    pub fn AllocateDescriptorPool(
        error: &Arc<Mutex<Error>>,
        device: &ash::Device,
        DescriptorPools: &mut SDeviceDescriptorPools,
        AllocPoolSize: usize,
    ) -> bool {
        let mut NewPool = SDeviceDescriptorPool::default();
        NewPool.size = AllocPoolSize as u64;

        let mut PoolSize = vk::DescriptorPoolSize::default();
        if DescriptorPools.is_uniform_pool {
            PoolSize.ty = vk::DescriptorType::UNIFORM_BUFFER;
        } else {
            PoolSize.ty = vk::DescriptorType::COMBINED_IMAGE_SAMPLER;
        }
        PoolSize.descriptor_count = AllocPoolSize as u32;

        let mut PoolInfo = vk::DescriptorPoolCreateInfo::default();
        PoolInfo.pool_size_count = 1;
        PoolInfo.p_pool_sizes = &PoolSize;
        PoolInfo.max_sets = AllocPoolSize as u32;
        PoolInfo.flags = vk::DescriptorPoolCreateFlags::FREE_DESCRIPTOR_SET;

        let res = unsafe { device.create_descriptor_pool(&PoolInfo, None) };
        if res.is_err() {
            error.lock().unwrap().SetError(
                EGFXErrorType::Init,
                localizable("Creating the descriptor pool failed."),
            );
            return false;
        }
        NewPool.pool = res.unwrap();

        DescriptorPools.pools.push(NewPool);

        return true;
    }

    #[must_use]
    pub fn GetDescriptorPoolForAlloc(
        error: &Arc<Mutex<Error>>,
        device: &ash::Device,
        RetDescr: &mut vk::DescriptorPool,
        DescriptorPools: &mut SDeviceDescriptorPools,
        pSets: *mut SDeviceDescriptorSet,
        AllocNum: usize,
    ) -> bool {
        let mut CurAllocNum = AllocNum;
        let mut CurAllocOffset = 0;
        *RetDescr = vk::DescriptorPool::null();

        while CurAllocNum > 0 {
            let mut AllocatedInThisRun = 0;

            let mut Found = false;
            let mut DescriptorPoolIndex = usize::MAX;
            for i in 0..DescriptorPools.pools.len() {
                let Pool = &mut DescriptorPools.pools[i];
                if Pool.cur_size + (CurAllocNum as u64) < Pool.size {
                    AllocatedInThisRun = CurAllocNum;
                    Pool.cur_size += CurAllocNum as u64;
                    Found = true;
                    if *RetDescr == vk::DescriptorPool::null() {
                        *RetDescr = Pool.pool;
                    }
                    DescriptorPoolIndex = i;
                    break;
                } else {
                    let RemainingPoolCount = Pool.size - Pool.cur_size;
                    if RemainingPoolCount > 0 {
                        AllocatedInThisRun = RemainingPoolCount as usize;
                        Pool.cur_size += RemainingPoolCount;
                        Found = true;
                        if *RetDescr == vk::DescriptorPool::null() {
                            *RetDescr = Pool.pool;
                        }
                        DescriptorPoolIndex = i;
                        break;
                    }
                }
            }

            if !Found {
                DescriptorPoolIndex = DescriptorPools.pools.len();

                if !Self::AllocateDescriptorPool(
                    error,
                    device,
                    DescriptorPools,
                    DescriptorPools.default_alloc_size as usize,
                ) {
                    return false;
                }

                AllocatedInThisRun =
                    std::cmp::min(DescriptorPools.default_alloc_size as usize, CurAllocNum);

                let Pool = DescriptorPools.pools.last_mut().unwrap();
                Pool.cur_size += AllocatedInThisRun as u64;
                if *RetDescr == vk::DescriptorPool::null() {
                    *RetDescr = Pool.pool;
                }
            }

            for i in CurAllocOffset..CurAllocOffset + AllocatedInThisRun {
                unsafe {
                    (*pSets.offset(i as isize)).pools = DescriptorPools;
                }
                unsafe {
                    (*pSets.offset(i as isize)).pool_index = DescriptorPoolIndex;
                }
            }
            CurAllocOffset += AllocatedInThisRun;
            CurAllocNum -= AllocatedInThisRun;
        }

        return true;
    }

    #[must_use]
    pub fn CreateNewTexturedStandardDescriptorSets(
        &mut self,
        _TextureSlot: usize,
        DescrIndex: usize,
        texture: &mut CTexture,
    ) -> bool {
        let DescrSet = &mut texture.vk_standard_textured_descr_sets[DescrIndex];

        let mut DesAllocInfo = vk::DescriptorSetAllocateInfo::default();
        if !Self::GetDescriptorPoolForAlloc(
            &self.error,
            &self.device,
            &mut DesAllocInfo.descriptor_pool,
            &mut self.standard_texture_descr_pool,
            DescrSet,
            1,
        ) {
            return false;
        }
        DesAllocInfo.descriptor_set_count = 1;
        DesAllocInfo.p_set_layouts = &self.standard_textured_descriptor_set_layout;

        let res = unsafe { self.device.allocate_descriptor_sets(&DesAllocInfo) };
        if res.is_err() {
            return false;
        }
        DescrSet.descriptor = res.unwrap()[0]; // TODO: array access

        let mut ImageInfo = vk::DescriptorImageInfo::default();
        ImageInfo.image_layout = vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL;
        ImageInfo.image_view = texture.img_view;
        ImageInfo.sampler = texture.samplers[DescrIndex];

        let mut aDescriptorWrites: [vk::WriteDescriptorSet; 1] = Default::default();
        aDescriptorWrites[0].dst_set = DescrSet.descriptor;
        aDescriptorWrites[0].dst_binding = 0;
        aDescriptorWrites[0].dst_array_element = 0;
        aDescriptorWrites[0].descriptor_type = vk::DescriptorType::COMBINED_IMAGE_SAMPLER;
        aDescriptorWrites[0].descriptor_count = 1;
        aDescriptorWrites[0].p_image_info = &ImageInfo;

        unsafe {
            self.device
                .update_descriptor_sets(aDescriptorWrites.as_slice(), &[]);
        }

        return true;
    }

    pub fn DestroyTexturedStandardDescriptorSets(
        device: &ash::Device,
        Texture: &mut CTexture,
        DescrIndex: usize,
    ) {
        let DescrSet = &mut Texture.vk_standard_textured_descr_sets[DescrIndex];
        if DescrSet.pool_index != usize::MAX {
            unsafe {
                device.free_descriptor_sets(
                    (*DescrSet.pools).pools[DescrSet.pool_index].pool,
                    &[DescrSet.descriptor],
                );
            }
        }
        *DescrSet = Default::default();
    }

    #[must_use]
    pub fn CreateNew3DTexturedStandardDescriptorSets(
        &mut self,
        _TextureSlot: usize,
        texture: &mut CTexture,
    ) -> bool {
        let DescrSet = &mut texture.vk_standard_3d_textured_descr_set;

        let mut DesAllocInfo = vk::DescriptorSetAllocateInfo::default();
        if !Self::GetDescriptorPoolForAlloc(
            &self.error,
            &self.device,
            &mut DesAllocInfo.descriptor_pool,
            &mut self.standard_texture_descr_pool,
            DescrSet,
            1,
        ) {
            return false;
        }
        DesAllocInfo.descriptor_set_count = 1;
        DesAllocInfo.p_set_layouts = &mut self.standard_3d_textured_descriptor_set_layout;

        let res = unsafe { self.device.allocate_descriptor_sets(&DesAllocInfo) };
        if res.is_err() {
            return false;
        }
        DescrSet.descriptor = res.unwrap()[0]; // TODO array access

        let mut ImageInfo = vk::DescriptorImageInfo::default();
        ImageInfo.image_layout = vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL;
        ImageInfo.image_view = texture.img_3d_view;
        ImageInfo.sampler = texture.sampler_3d;

        let mut aDescriptorWrites: [vk::WriteDescriptorSet; 1] = Default::default();

        aDescriptorWrites[0].dst_set = DescrSet.descriptor;
        aDescriptorWrites[0].dst_binding = 0;
        aDescriptorWrites[0].dst_array_element = 0;
        aDescriptorWrites[0].descriptor_type = vk::DescriptorType::COMBINED_IMAGE_SAMPLER;
        aDescriptorWrites[0].descriptor_count = 1;
        aDescriptorWrites[0].p_image_info = &ImageInfo;

        unsafe {
            self.device
                .update_descriptor_sets(aDescriptorWrites.as_slice(), &[]);
        }

        return true;
    }

    pub fn DestroyTextured3DStandardDescriptorSets(device: &ash::Device, Texture: &mut CTexture) {
        let DescrSet = &mut Texture.vk_standard_3d_textured_descr_set;
        if DescrSet.pool_index != usize::MAX {
            unsafe {
                device.free_descriptor_sets(
                    (*DescrSet.pools).pools[DescrSet.pool_index].pool,
                    &[DescrSet.descriptor],
                );
            }
        }
    }

    #[must_use]
    pub fn CreateNewTextDescriptorSets(&mut self, Texture: usize, TextureOutline: usize) -> bool {
        let TextureText = &mut self.textures[Texture];
        let DescrSetText = &mut TextureText.vk_text_descr_set;

        let mut DesAllocInfo = vk::DescriptorSetAllocateInfo::default();
        if !Self::GetDescriptorPoolForAlloc(
            &self.error,
            &self.device,
            &mut DesAllocInfo.descriptor_pool,
            &mut self.text_texture_descr_pool,
            DescrSetText,
            1,
        ) {
            return false;
        }
        DesAllocInfo.descriptor_set_count = 1;
        DesAllocInfo.p_set_layouts = &self.text_descriptor_set_layout;

        let res = unsafe { self.device.allocate_descriptor_sets(&DesAllocInfo) };
        if res.is_err() {
            return false;
        }

        DescrSetText.descriptor = res.unwrap()[0];
        let mut aDescriptorWrites: [vk::WriteDescriptorSet; 2] = Default::default();
        aDescriptorWrites[0].dst_set = DescrSetText.descriptor;

        let mut aImageInfo: [vk::DescriptorImageInfo; 2] = Default::default();
        aImageInfo[0].image_layout = vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL;
        aImageInfo[0].image_view = TextureText.img_view;
        aImageInfo[0].sampler = TextureText.samplers[0];
        aImageInfo[1].image_layout = vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL;
        let TextureTextOutline = &mut self.textures[TextureOutline];
        aImageInfo[1].image_view = TextureTextOutline.img_view;
        aImageInfo[1].sampler = TextureTextOutline.samplers[0];

        aDescriptorWrites[0].dst_binding = 0;
        aDescriptorWrites[0].dst_array_element = 0;
        aDescriptorWrites[0].descriptor_type = vk::DescriptorType::COMBINED_IMAGE_SAMPLER;
        aDescriptorWrites[0].descriptor_count = 1;
        aDescriptorWrites[0].p_image_info = aImageInfo.as_ptr();
        aDescriptorWrites[1] = aDescriptorWrites[0];
        aDescriptorWrites[1].dst_binding = 1;
        aDescriptorWrites[1].p_image_info = &aImageInfo[1];

        unsafe {
            self.device
                .update_descriptor_sets(aDescriptorWrites.as_slice(), &[]);
        }

        return true;
    }

    pub fn DestroyTextDescriptorSets(
        device: &ash::Device,
        Texture: &mut CTexture,
        _TextureOutline: &mut CTexture,
    ) {
        let DescrSet = &mut Texture.vk_text_descr_set;
        if DescrSet.pool_index != usize::MAX {
            unsafe {
                device.free_descriptor_sets(
                    (*DescrSet.pools).pools[DescrSet.pool_index].pool,
                    &[DescrSet.descriptor],
                );
            }
        }
    }

    #[must_use]
    pub fn CreateUniformDescriptorSetLayout(
        error: &Arc<Mutex<Error>>,
        device: &ash::Device,
        SetLayout: &mut vk::DescriptorSetLayout,
        StageFlags: vk::ShaderStageFlags,
    ) -> bool {
        let mut SamplerLayoutBinding = vk::DescriptorSetLayoutBinding::default();
        SamplerLayoutBinding.binding = 1;
        SamplerLayoutBinding.descriptor_count = 1;
        SamplerLayoutBinding.descriptor_type = vk::DescriptorType::UNIFORM_BUFFER;
        SamplerLayoutBinding.p_immutable_samplers = std::ptr::null();
        SamplerLayoutBinding.stage_flags = StageFlags;

        let aBindings = [SamplerLayoutBinding];
        let mut LayoutInfo = vk::DescriptorSetLayoutCreateInfo::default();
        LayoutInfo.binding_count = aBindings.len() as u32;
        LayoutInfo.p_bindings = aBindings.as_ptr();

        let res = unsafe { device.create_descriptor_set_layout(&LayoutInfo, None) };
        if res.is_err() {
            error.lock().unwrap().SetError(
                EGFXErrorType::Init,
                localizable("Creating descriptor layout failed."),
            );
            return false;
        }
        *SetLayout = res.unwrap();
        return true;
    }

    #[must_use]
    pub fn CreateSpriteMultiUniformDescriptorSetLayout(&mut self) -> bool {
        return Self::CreateUniformDescriptorSetLayout(
            &self.error,
            &self.device,
            &mut self.sprite_multi_uniform_descriptor_set_layout,
            vk::ShaderStageFlags::VERTEX,
        );
    }

    #[must_use]
    pub fn CreateQuadUniformDescriptorSetLayout(&mut self) -> bool {
        return Self::CreateUniformDescriptorSetLayout(
            &self.error,
            &self.device,
            &mut self.quad_uniform_descriptor_set_layout,
            vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
        );
    }

    pub fn DestroyUniformDescriptorSetLayouts(&mut self) {
        unsafe {
            self.device
                .destroy_descriptor_set_layout(self.quad_uniform_descriptor_set_layout, None);
        }
        unsafe {
            self.device.destroy_descriptor_set_layout(
                self.sprite_multi_uniform_descriptor_set_layout,
                None,
            );
        }
    }

    #[must_use]
    pub fn CreateUniformDescriptorSets(
        error: &Arc<Mutex<Error>>,
        device: &ash::Device,
        DescrPools: &mut SDeviceDescriptorPools,
        SetLayout: &vk::DescriptorSetLayout,
        pSets: *mut SDeviceDescriptorSet,
        SetCount: usize,
        BindBuffer: vk::Buffer,
        SingleBufferInstanceSize: usize,
        MemoryOffset: vk::DeviceSize,
    ) -> bool {
        let mut RetDescr = vk::DescriptorPool::default();
        if !Self::GetDescriptorPoolForAlloc(
            error,
            device,
            &mut RetDescr,
            DescrPools,
            pSets,
            SetCount,
        ) {
            return false;
        }
        let mut DesAllocInfo = vk::DescriptorSetAllocateInfo::default();
        DesAllocInfo.descriptor_set_count = 1;
        DesAllocInfo.p_set_layouts = SetLayout;
        for i in 0..SetCount {
            DesAllocInfo.descriptor_pool = unsafe {
                (*(*pSets.offset(i as isize)).pools).pools[(*pSets.offset(i as isize)).pool_index]
                    .pool
            };
            let res = unsafe { device.allocate_descriptor_sets(&DesAllocInfo) };
            if res.is_err() {
                return false;
            }
            unsafe {
                (*pSets.offset(i as isize)).descriptor = res.unwrap()[0];
            } // TODO [0] right?

            let mut buffer_info = vk::DescriptorBufferInfo::default();
            buffer_info.buffer = BindBuffer;
            buffer_info.offset = MemoryOffset + (SingleBufferInstanceSize * i) as u64;
            buffer_info.range = SingleBufferInstanceSize as u64;

            let mut descriptor_writes: [vk::WriteDescriptorSet; 1] = Default::default();
            descriptor_writes[0].dst_set = unsafe { (*pSets.offset(i as isize)).descriptor };
            descriptor_writes[0].dst_binding = 1;
            descriptor_writes[0].dst_array_element = 0;
            descriptor_writes[0].descriptor_type = vk::DescriptorType::UNIFORM_BUFFER;
            descriptor_writes[0].descriptor_count = 1;
            descriptor_writes[0].p_buffer_info = &buffer_info;

            unsafe {
                device.update_descriptor_sets(&descriptor_writes, &[]);
            }
        }

        return true;
    }

    pub fn DestroyUniformDescriptorSets(
        device: &ash::Device,
        pSets: *mut SDeviceDescriptorSet,
        SetCount: usize,
    ) {
        for i in 0..SetCount {
            unsafe {
                device.free_descriptor_sets(
                    (*(*pSets.offset(i as isize)).pools).pools
                        [(*pSets.offset(i as isize)).pool_index]
                        .pool,
                    &[(*pSets.offset(i as isize)).descriptor],
                );
            }
            unsafe {
                (*pSets.offset(i as isize)).descriptor = vk::DescriptorSet::null();
            }
        }
    }
}
