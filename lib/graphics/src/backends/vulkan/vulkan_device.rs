use std::{
    collections::BTreeMap,
    sync::{
        atomic::{AtomicU64, AtomicU8},
        Arc, Mutex,
    },
};

use ash::vk;
use graphics_types::{
    command_buffer::{GlVertexTex3DStream, StreamDataMax},
    types::GraphicsBackendMemory,
};
use libc::c_void;

use super::{
    common::{localizable, EGFXErrorType},
    vulkan_allocator::VulkanAllocator,
    vulkan_config::Config,
    vulkan_dbg::is_verbose,
    vulkan_error::Error,
    vulkan_limits::Limits,
    vulkan_mem::Memory,
    vulkan_types::{
        CTexture, EMemoryBlockUsage, ESupportedSamplerTypes, SBufferContainer, SBufferObjectFrame,
        SDeviceDescriptorPool, SDeviceDescriptorPools, SDeviceDescriptorSet, SDeviceMemoryBlock,
        SFrameBuffers, SFrameUniformBuffers, SMemoryBlock, SMemoryBlockCache,
        SMemoryHeapQueueElement, SMemoryImageBlock, SStreamMemory, StreamMemory,
        VKDelayedBufferCleanupItem, IMAGE_BUFFER_CACHE_ID, STAGING_BUFFER_CACHE_ID,
        STAGING_BUFFER_IMAGE_CACHE_ID, VERTEX_BUFFER_CACHE_ID,
    },
};

// good approximation of 1024x1024 image with mipmaps
const IMG_SIZE1024X1024: i64 = (1024 * 1024 * 4) * 2;

pub struct Device {
    dbg: Arc<AtomicU8>, // @see EDebugGFXModes
    pub mem: Memory,
    pub mem_allocator: Arc<std::sync::Mutex<Option<VulkanAllocator>>>,

    _instance: ash::Instance,
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
    pub config: Config,

    pub non_flushed_staging_buffer_ranges: Vec<vk::MappedMemoryRange>,

    pub frame_delayed_buffer_cleanups: Vec<Vec<VKDelayedBufferCleanupItem>>,
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
        config: Config,
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
            _instance: instance.clone(),
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
            config: config,
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
            .get_buffer_block_impl::<{ STAGING_BUFFER_CACHE_ID }, { 8 * 1024 * 1024 }, 3, true>(
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
            .get_buffer_block_impl::<{ STAGING_BUFFER_IMAGE_CACHE_ID }, { 8 * 1024 * 1024 }, 3, true>(
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
    pub fn get_image_memory(
        &mut self,
        ret_block: &mut SMemoryImageBlock<IMAGE_BUFFER_CACHE_ID>,
        required_size: vk::DeviceSize,
        required_alignment: vk::DeviceSize,
        required_memory_type_bits: u32,
    ) -> bool {
        let mut it = self.image_buffer_caches.get_mut(&required_memory_type_bits);
        let mem: &mut SMemoryBlockCache<{ IMAGE_BUFFER_CACHE_ID }>;
        if it.is_none() {
            self.image_buffer_caches
                .insert(required_memory_type_bits, SMemoryBlockCache::default());
            it = self.image_buffer_caches.get_mut(&required_memory_type_bits);

            mem = it.unwrap();
            mem.init(self.swap_chain_image_count as usize);
        } else {
            mem = it.unwrap();
        }
        return self
            .mem
            .get_image_memory_block_impl::<IMAGE_BUFFER_CACHE_ID, IMG_SIZE1024X1024, 2>(
                ret_block,
                mem,
                vk::MemoryPropertyFlags::DEVICE_LOCAL,
                required_size,
                required_alignment,
                required_memory_type_bits,
            );
    }

    pub fn free_image_mem_block(
        frame_delay_buffer_cleanup: &mut Vec<Vec<VKDelayedBufferCleanupItem>>,
        image_buffer_caches: &mut BTreeMap<u32, SMemoryBlockCache<{ IMAGE_BUFFER_CACHE_ID }>>,
        block: &SMemoryImageBlock<IMAGE_BUFFER_CACHE_ID>,
        cur_image_index: u32,
    ) {
        if !block.base.is_cached {
            frame_delay_buffer_cleanup[cur_image_index as usize].push(VKDelayedBufferCleanupItem {
                buffer: block.base.buffer,
                mem: block.base.buffer_mem.clone(),
                mapped_data: std::ptr::null_mut(),
            });
        } else {
            image_buffer_caches
                .get_mut(&block.image_memory_bits)
                .unwrap()
                .free_mem_block(&block.base, cur_image_index as usize);
        }
    }

    pub fn upload_streamed_buffer<const FLUSH_FOR_RENDERING: bool, TName>(
        device: &ash::Device,
        non_coherent_mem_alignment: u64,
        streamed_buffer: &mut SStreamMemory<TName>,
        cur_image_index: u32,
    ) where
        TName: Clone + StreamMemory + Default,
    {
        let mut range_update_count: usize = 0;
        if streamed_buffer.is_used(cur_image_index as usize) {
            for i in 0..streamed_buffer.get_used_count(cur_image_index as usize) {
                let (buffers_of_frame, mem_ranges) =
                    streamed_buffer.get_buffers_and_ranges(cur_image_index as usize);
                let (buffer_of_frame, mem_range) = (
                    &mut buffers_of_frame[i],
                    &mut mem_ranges[range_update_count],
                );
                range_update_count += 1;
                mem_range.s_type = vk::StructureType::MAPPED_MEMORY_RANGE;
                mem_range.memory = buffer_of_frame.get_device_mem_block().mem;
                mem_range.offset = *buffer_of_frame.get_offset_in_buffer() as u64;
                let alignment_mod =
                    *buffer_of_frame.get_used_size() as vk::DeviceSize % non_coherent_mem_alignment;
                let mut alignment_req = non_coherent_mem_alignment - alignment_mod;
                if alignment_mod == 0 {
                    alignment_req = 0;
                }
                mem_range.size = *buffer_of_frame.get_used_size() as u64 + alignment_req;

                if mem_range.offset + mem_range.size > buffer_of_frame.get_device_mem_block().size {
                    mem_range.size = vk::WHOLE_SIZE;
                }

                buffer_of_frame.reset_is_used();
            }
            if range_update_count > 0 && FLUSH_FOR_RENDERING {
                unsafe {
                    device.flush_mapped_memory_ranges(
                        streamed_buffer
                            .get_ranges(cur_image_index as usize)
                            .split_at(range_update_count)
                            .0,
                    );
                }
            }
        }
        streamed_buffer.reset_frame(cur_image_index as usize);
    }

    pub fn prepare_staging_mem_range_impl(
        &mut self,
        buffer_mem: &SDeviceMemoryBlock,
        heap_data: &SMemoryHeapQueueElement,
    ) {
        let mut upload_range = vk::MappedMemoryRange::default();
        upload_range.memory = buffer_mem.mem;
        upload_range.offset = heap_data.offset_to_align as u64;

        let alignment_mod =
            heap_data.allocation_size as vk::DeviceSize % self.limits.non_coherent_mem_alignment;
        let mut alignment_req = self.limits.non_coherent_mem_alignment - alignment_mod;
        if alignment_mod == 0 {
            alignment_req = 0;
        }
        upload_range.size = (heap_data.allocation_size as u64 + alignment_req) as u64;

        if upload_range.offset + upload_range.size > buffer_mem.size {
            upload_range.size = vk::WHOLE_SIZE;
        }

        self.non_flushed_staging_buffer_ranges.push(upload_range);
    }

    pub fn prepare_staging_mem_range<const ID: usize>(&mut self, block: &mut SMemoryBlock<ID>) {
        self.prepare_staging_mem_range_impl(&block.buffer_mem, &block.heap_data);
    }

    pub fn upload_and_free_staging_mem_block(
        &mut self,
        block: &mut SMemoryBlock<STAGING_BUFFER_CACHE_ID>,
        cur_image_index: u32,
    ) {
        self.prepare_staging_mem_range(block);
        if !block.is_cached {
            self.frame_delayed_buffer_cleanups[cur_image_index as usize].push(
                VKDelayedBufferCleanupItem {
                    buffer: block.buffer,
                    mem: block.buffer_mem.clone(),
                    mapped_data: block.mapped_buffer,
                },
            );
        } else {
            self.staging_buffer_cache
                .free_mem_block(block, cur_image_index as usize);
        }
    }

    pub fn upload_and_free_staging_image_mem_block(
        &mut self,
        block: &mut SMemoryBlock<STAGING_BUFFER_IMAGE_CACHE_ID>,
        cur_image_index: u32,
    ) {
        self.prepare_staging_mem_range(block);
        if !block.is_cached {
            self.frame_delayed_buffer_cleanups[cur_image_index as usize].push(
                VKDelayedBufferCleanupItem {
                    buffer: block.buffer,
                    mem: block.buffer_mem.clone(),
                    mapped_data: block.mapped_buffer,
                },
            );
        } else {
            self.staging_buffer_cache_image
                .free_mem_block(block, cur_image_index as usize);
        }
    }

    #[must_use]
    pub fn get_vertex_buffer(
        &mut self,
        res_block: &mut SMemoryBlock<VERTEX_BUFFER_CACHE_ID>,
        required_size: vk::DeviceSize,
    ) -> bool {
        return self
            .mem
            .get_buffer_block_impl::<{ VERTEX_BUFFER_CACHE_ID }, { 8 * 1024 * 1024 }, 3, false>(
                res_block,
                &mut self.vertex_buffer_cache,
                vk::BufferUsageFlags::TRANSFER_DST | vk::BufferUsageFlags::VERTEX_BUFFER,
                vk::MemoryPropertyFlags::DEVICE_LOCAL,
                std::ptr::null(),
                required_size,
                16,
            );
    }

    pub fn free_vertex_mem_block(
        frame_delayed_buffer_cleanup: &mut Vec<Vec<VKDelayedBufferCleanupItem>>,
        vertex_buffer_cache: &mut SMemoryBlockCache<{ VERTEX_BUFFER_CACHE_ID }>,
        block: &SMemoryBlock<VERTEX_BUFFER_CACHE_ID>,
        cur_image_index: u32,
    ) {
        if !block.is_cached {
            frame_delayed_buffer_cleanup[cur_image_index as usize].push(
                VKDelayedBufferCleanupItem {
                    buffer: block.buffer,
                    mem: block.buffer_mem.clone(),
                    mapped_data: std::ptr::null_mut(),
                },
            );
        } else {
            vertex_buffer_cache.free_mem_block(block, cur_image_index as usize);
        }
    }

    pub fn destroy_texture(
        frame_delay_buffer_cleanup: &mut Vec<Vec<VKDelayedBufferCleanupItem>>,
        image_buffer_caches: &mut BTreeMap<u32, SMemoryBlockCache<{ IMAGE_BUFFER_CACHE_ID }>>,
        device: &ash::Device,
        texture: &mut CTexture,
        cur_image_index: u32,
    ) {
        if texture.img != vk::Image::null() {
            Self::free_image_mem_block(
                frame_delay_buffer_cleanup,
                image_buffer_caches,
                &mut texture.img_mem,
                cur_image_index,
            );
            unsafe {
                device.destroy_image(texture.img, None);
            }

            unsafe {
                device.destroy_image_view(texture.img_view, None);
            }
        }

        if texture.img_3d != vk::Image::null() {
            Self::free_image_mem_block(
                frame_delay_buffer_cleanup,
                image_buffer_caches,
                &mut texture.img_3d_mem,
                cur_image_index,
            );
            unsafe {
                device.destroy_image(texture.img_3d, None);
            }

            unsafe {
                device.destroy_image_view(texture.img_3d_view, None);
            }
        }

        Self::destroy_textured_standard_descriptor_sets(&device, texture, 0);
        Self::destroy_textured_standard_descriptor_sets(&device, texture, 1);

        Self::destroy_textured_3d_standard_descriptor_sets(&device, texture);
    }

    pub fn destroy_text_texture(
        frame_delay_buffer_cleanup: &mut Vec<Vec<VKDelayedBufferCleanupItem>>,
        image_buffer_caches: &mut BTreeMap<u32, SMemoryBlockCache<{ IMAGE_BUFFER_CACHE_ID }>>,
        device: &ash::Device,
        texture: &mut CTexture,
        texture_outline: &mut CTexture,
        cur_image_index: u32,
    ) {
        if texture.img != vk::Image::null() {
            Self::free_image_mem_block(
                frame_delay_buffer_cleanup,
                image_buffer_caches,
                &mut texture.img_mem,
                cur_image_index,
            );
            unsafe {
                device.destroy_image(texture.img, None);
            }

            unsafe {
                device.destroy_image_view(texture.img_view, None);
            }
        }

        if texture_outline.img != vk::Image::null() {
            Self::free_image_mem_block(
                frame_delay_buffer_cleanup,
                image_buffer_caches,
                &mut texture_outline.img_mem,
                cur_image_index,
            );
            unsafe {
                device.destroy_image(texture_outline.img, None);
            }

            unsafe {
                device.destroy_image_view(texture_outline.img_view, None);
            }
        }

        Self::destroy_text_descriptor_sets(device, texture, texture_outline);
    }

    pub fn shrink_unused_caches(&mut self) {
        let mut freeed_memory: usize = 0;
        freeed_memory += self.staging_buffer_cache.shrink(&mut self.device);
        freeed_memory += self.staging_buffer_cache_image.shrink(&mut self.device);
        if freeed_memory > 0 {
            self.staging_memory_usage
                .fetch_sub(freeed_memory as u64, std::sync::atomic::Ordering::Relaxed);
            if is_verbose(&*self.dbg) {
                // TODO dbg_msg("vulkan", "deallocated chunks of memory with size: %" PRIu64 " from all frames (staging buffer)", (usize)FreeedMemory);
            }
        }
        freeed_memory = 0;
        freeed_memory += self.vertex_buffer_cache.shrink(&mut self.device);
        if freeed_memory > 0 {
            self.buffer_memory_usage
                .fetch_sub(freeed_memory as u64, std::sync::atomic::Ordering::Relaxed);
            if is_verbose(&*self.dbg) {
                // TODO dbg_msg("vulkan", "deallocated chunks of memory with size: %" PRIu64 " from all frames (buffer)", (usize)FreeedMemory);
            }
        }
        freeed_memory = 0;
        for image_buffer_cache in &mut self.image_buffer_caches {
            freeed_memory += image_buffer_cache.1.shrink(&mut self.device);
        }
        if freeed_memory > 0 {
            self.texture_memory_usage
                .fetch_sub(freeed_memory as u64, std::sync::atomic::Ordering::Relaxed);
            if is_verbose(&*self.dbg) {
                // TODO dbg_msg("vulkan", "deallocated chunks of memory with size: %" PRIu64 " from all frames (texture)", (usize)FreeedMemory);
            }
        }
    }

    #[must_use]
    pub fn get_memory_command_buffer(
        &mut self,
        res_mem_command_buffer: &mut *mut vk::CommandBuffer,
        cur_image_index: u32,
    ) -> bool {
        let memory_command_buffer = &mut self.memory_command_buffers[cur_image_index as usize];
        if !self.used_memory_command_buffer[cur_image_index as usize] {
            self.used_memory_command_buffer[cur_image_index as usize] = true;

            if unsafe {
                self.device.reset_command_buffer(
                    *memory_command_buffer,
                    vk::CommandBufferResetFlags::RELEASE_RESOURCES,
                )
            }
            .is_err()
            {
                self.error.lock().unwrap().set_error(
                    EGFXErrorType::RenderRecording,
                    localizable("Command buffer cannot be resetted anymore."),
                );
                return false;
            }

            let mut begin_info = vk::CommandBufferBeginInfo::default();
            begin_info.flags = vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT;
            unsafe {
                if self
                    .device
                    .begin_command_buffer(*memory_command_buffer, &begin_info)
                    .is_err()
                {
                    self.error.lock().unwrap().set_error(
                        EGFXErrorType::RenderRecording,
                        localizable("Command buffer cannot be filled anymore."),
                    );
                    return false;
                }
            }
        }
        *res_mem_command_buffer = memory_command_buffer;
        return true;
    }

    #[must_use]
    pub fn memory_barrier(
        &mut self,
        buffer: vk::Buffer,
        offset: vk::DeviceSize,
        size: vk::DeviceSize,
        buffer_access_type: vk::AccessFlags,
        before_command: bool,
        cur_image_index: u32,
    ) -> bool {
        let mut ptr_mem_command_buffer: *mut vk::CommandBuffer = std::ptr::null_mut();
        if !self.get_memory_command_buffer(&mut ptr_mem_command_buffer, cur_image_index) {
            return false;
        }
        let mem_command_buffer = unsafe { &mut *ptr_mem_command_buffer };

        let mut barrier = vk::BufferMemoryBarrier::default();
        barrier.src_queue_family_index = vk::QUEUE_FAMILY_IGNORED;
        barrier.dst_queue_family_index = vk::QUEUE_FAMILY_IGNORED;
        barrier.buffer = buffer;
        barrier.offset = offset;
        barrier.size = size;

        let source_stage;
        let destination_stage;

        if before_command {
            barrier.src_access_mask = buffer_access_type;
            barrier.dst_access_mask = vk::AccessFlags::TRANSFER_WRITE;

            source_stage = vk::PipelineStageFlags::VERTEX_INPUT;
            destination_stage = vk::PipelineStageFlags::TRANSFER;
        } else {
            barrier.src_access_mask = vk::AccessFlags::TRANSFER_WRITE;
            barrier.dst_access_mask = buffer_access_type;

            source_stage = vk::PipelineStageFlags::TRANSFER;
            destination_stage = vk::PipelineStageFlags::VERTEX_INPUT;
        }

        unsafe {
            self.device.cmd_pipeline_barrier(
                *mem_command_buffer,
                source_stage,
                destination_stage,
                vk::DependencyFlags::empty(),
                &[],
                &[barrier],
                &[],
            );
        }

        return true;
    }

    /************************
     * TEXTURES
     ************************/

    #[must_use]
    pub fn build_mipmaps(
        &mut self,
        image: vk::Image,
        _image_format: vk::Format,
        width: usize,
        height: usize,
        depth: usize,
        mip_map_level_count: usize,
        cur_image_index: u32,
    ) -> bool {
        let mut ptr_mem_command_buffer: *mut vk::CommandBuffer = std::ptr::null_mut();
        if !self.get_memory_command_buffer(&mut ptr_mem_command_buffer, cur_image_index) {
            return false;
        }
        let mem_command_buffer = unsafe { &mut *ptr_mem_command_buffer };

        let mut barrier = vk::ImageMemoryBarrier::default();
        barrier.image = image;
        barrier.src_queue_family_index = vk::QUEUE_FAMILY_IGNORED;
        barrier.dst_queue_family_index = vk::QUEUE_FAMILY_IGNORED;
        barrier.subresource_range.aspect_mask = vk::ImageAspectFlags::COLOR;
        barrier.subresource_range.level_count = 1;
        barrier.subresource_range.base_array_layer = 0;
        barrier.subresource_range.layer_count = depth as u32;

        let mut tmp_mip_width: i32 = width as i32;
        let mut tmp_mip_height: i32 = height as i32;

        for i in 1..mip_map_level_count {
            barrier.subresource_range.base_mip_level = (i - 1) as u32;
            barrier.old_layout = vk::ImageLayout::TRANSFER_DST_OPTIMAL;
            barrier.new_layout = vk::ImageLayout::TRANSFER_SRC_OPTIMAL;
            barrier.src_access_mask = vk::AccessFlags::TRANSFER_WRITE;
            barrier.dst_access_mask = vk::AccessFlags::TRANSFER_READ;

            unsafe {
                self.device.cmd_pipeline_barrier(
                    *mem_command_buffer,
                    vk::PipelineStageFlags::TRANSFER,
                    vk::PipelineStageFlags::TRANSFER,
                    vk::DependencyFlags::empty(),
                    &[],
                    &[],
                    &[barrier],
                );
            }

            let mut blit = vk::ImageBlit::default();
            blit.src_offsets[0] = vk::Offset3D::default();
            blit.src_offsets[1] = vk::Offset3D {
                x: tmp_mip_width,
                y: tmp_mip_height,
                z: 1,
            };
            blit.src_subresource.aspect_mask = vk::ImageAspectFlags::COLOR;
            blit.src_subresource.mip_level = (i - 1) as u32;
            blit.src_subresource.base_array_layer = 0;
            blit.src_subresource.layer_count = depth as u32;
            blit.dst_offsets[0] = vk::Offset3D::default();
            blit.dst_offsets[1] = vk::Offset3D {
                x: if tmp_mip_width > 1 {
                    tmp_mip_width / 2
                } else {
                    1
                },
                y: if tmp_mip_height > 1 {
                    tmp_mip_height / 2
                } else {
                    1
                },
                z: 1,
            };
            blit.dst_subresource.aspect_mask = vk::ImageAspectFlags::COLOR;
            blit.dst_subresource.mip_level = i as u32;
            blit.dst_subresource.base_array_layer = 0;
            blit.dst_subresource.layer_count = depth as u32;

            unsafe {
                self.device.cmd_blit_image(
                    *mem_command_buffer,
                    image,
                    vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                    image,
                    vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                    &[blit],
                    if self.allows_linear_blitting {
                        vk::Filter::LINEAR
                    } else {
                        vk::Filter::NEAREST
                    },
                );
            }

            barrier.old_layout = vk::ImageLayout::TRANSFER_SRC_OPTIMAL;
            barrier.new_layout = vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL;
            barrier.src_access_mask = vk::AccessFlags::TRANSFER_READ;
            barrier.dst_access_mask = vk::AccessFlags::SHADER_READ;

            unsafe {
                self.device.cmd_pipeline_barrier(
                    *mem_command_buffer,
                    vk::PipelineStageFlags::TRANSFER,
                    vk::PipelineStageFlags::FRAGMENT_SHADER,
                    vk::DependencyFlags::empty(),
                    &[],
                    &[],
                    &[barrier],
                );
            }

            if tmp_mip_width > 1 {
                tmp_mip_width /= 2;
            }
            if tmp_mip_height > 1 {
                tmp_mip_height /= 2;
            }
        }

        barrier.subresource_range.base_mip_level = (mip_map_level_count - 1) as u32;
        barrier.old_layout = vk::ImageLayout::TRANSFER_DST_OPTIMAL;
        barrier.new_layout = vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL;
        barrier.src_access_mask = vk::AccessFlags::TRANSFER_WRITE;
        barrier.dst_access_mask = vk::AccessFlags::SHADER_READ;

        unsafe {
            self.device.cmd_pipeline_barrier(
                *mem_command_buffer,
                vk::PipelineStageFlags::TRANSFER,
                vk::PipelineStageFlags::FRAGMENT_SHADER,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                &[barrier],
            );
        }

        return true;
    }

    #[must_use]
    pub fn create_texture_image(
        &mut self,
        _image_index: usize,
        new_image: &mut vk::Image,
        new_img_mem: &mut SMemoryImageBlock<IMAGE_BUFFER_CACHE_ID>,
        mut upload_data: GraphicsBackendMemory,
        format: vk::Format,
        width: usize,
        height: usize,
        depth: usize,
        _pixel_size: usize,
        mip_map_level_count: usize,
        cur_image_index: u32,
    ) -> bool {
        let allocator_dummy = self.mem_allocator.clone();
        let mut allocator_res = allocator_dummy.lock().unwrap();
        let allocator = allocator_res.as_mut().unwrap();
        let buffer_block_res =
            allocator.get_mem_block_image(upload_data.as_mut_slice().as_ptr() as *mut c_void);

        let staging_buffer = buffer_block_res.unwrap();

        let img_format = format;

        if !self.create_image(
            width as u32,
            height as u32,
            depth as u32,
            mip_map_level_count,
            img_format,
            vk::ImageTiling::OPTIMAL,
            new_image,
            new_img_mem,
        ) {
            return false;
        }

        if !self.image_barrier(
            *new_image,
            0,
            mip_map_level_count,
            0,
            depth,
            img_format,
            vk::ImageLayout::UNDEFINED,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            cur_image_index,
        ) {
            return false;
        }
        if !self.copy_buffer_to_image(
            staging_buffer.buffer,
            staging_buffer.heap_data.offset_to_align as u64,
            *new_image,
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
            self.prepare_staging_mem_range_impl(block, queue_el);
        });

        if mip_map_level_count > 1 {
            if !self.build_mipmaps(
                *new_image,
                img_format,
                width,
                height,
                depth,
                mip_map_level_count,
                cur_image_index,
            ) {
                return false;
            }
        } else {
            if !self.image_barrier(
                *new_image,
                0,
                1,
                0,
                depth,
                img_format,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                cur_image_index,
            ) {
                return false;
            }
        }

        return true;
    }

    pub fn create_texture_image_view(
        &mut self,
        tex_image: vk::Image,
        img_format: vk::Format,
        view_type: vk::ImageViewType,
        depth: usize,
        mip_map_level_count: usize,
    ) -> anyhow::Result<vk::ImageView> {
        self.create_image_view(
            tex_image,
            img_format,
            view_type,
            depth,
            mip_map_level_count,
            vk::ImageAspectFlags::COLOR,
        )
    }

    #[must_use]
    pub fn create_texture_samplers_impl(
        device: &ash::Device,
        max_sampler_anisotropy: u32,
        global_texture_lod_bias: i32,
        created_sampler: &mut vk::Sampler,
        addr_mode_u: vk::SamplerAddressMode,
        addr_mode_v: vk::SamplerAddressMode,
        addr_mode_w: vk::SamplerAddressMode,
    ) -> bool {
        let mut sampler_info = vk::SamplerCreateInfo::default();
        sampler_info.mag_filter = vk::Filter::LINEAR;
        sampler_info.min_filter = vk::Filter::LINEAR;
        sampler_info.address_mode_u = addr_mode_u;
        sampler_info.address_mode_v = addr_mode_v;
        sampler_info.address_mode_w = addr_mode_w;
        sampler_info.anisotropy_enable = vk::FALSE;
        sampler_info.max_anisotropy = max_sampler_anisotropy as f32;
        sampler_info.border_color = vk::BorderColor::INT_OPAQUE_BLACK;
        sampler_info.unnormalized_coordinates = vk::FALSE;
        sampler_info.compare_enable = vk::FALSE;
        sampler_info.compare_op = vk::CompareOp::ALWAYS;
        sampler_info.mipmap_mode = vk::SamplerMipmapMode::LINEAR;
        sampler_info.mip_lod_bias = global_texture_lod_bias as f32 / 1000.0;
        sampler_info.min_lod = -1000.0;
        sampler_info.max_lod = 1000.0;

        let res = unsafe { device.create_sampler(&sampler_info, None) };
        if let Err(_) = res {
            // TODO dbg_msg("vulkan", "failed to create texture sampler!");
            return false;
        }
        *created_sampler = res.unwrap();
        return true;
    }

    pub fn create_image_view(
        &mut self,
        image: vk::Image,
        format: vk::Format,
        view_type: vk::ImageViewType,
        depth: usize,
        mip_map_level_count: usize,
        aspect_mask: vk::ImageAspectFlags,
    ) -> anyhow::Result<vk::ImageView> {
        let mut view_create_info = vk::ImageViewCreateInfo::default();
        view_create_info.image = image;
        view_create_info.view_type = view_type;
        view_create_info.format = format;
        view_create_info.subresource_range.aspect_mask = aspect_mask;
        view_create_info.subresource_range.base_mip_level = 0;
        view_create_info.subresource_range.level_count = mip_map_level_count as u32;
        view_create_info.subresource_range.base_array_layer = 0;
        view_create_info.subresource_range.layer_count = depth as u32;

        Ok(unsafe { self.device.create_image_view(&view_create_info, None) }?)
    }

    pub fn get_max_sample_count(limits: &Limits) -> vk::SampleCountFlags {
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

    pub fn get_sample_count(ms_count: u32, limits: &Limits) -> vk::SampleCountFlags {
        let max_sample_count = Self::get_max_sample_count(limits);
        if ms_count >= 64 && max_sample_count >= vk::SampleCountFlags::TYPE_64 {
            return vk::SampleCountFlags::TYPE_64;
        } else if ms_count >= 32 && max_sample_count >= vk::SampleCountFlags::TYPE_32 {
            return vk::SampleCountFlags::TYPE_32;
        } else if ms_count >= 16 && max_sample_count >= vk::SampleCountFlags::TYPE_16 {
            return vk::SampleCountFlags::TYPE_16;
        } else if ms_count >= 8 && max_sample_count >= vk::SampleCountFlags::TYPE_8 {
            return vk::SampleCountFlags::TYPE_8;
        } else if ms_count >= 4 && max_sample_count >= vk::SampleCountFlags::TYPE_4 {
            return vk::SampleCountFlags::TYPE_4;
        } else if ms_count >= 2 && max_sample_count >= vk::SampleCountFlags::TYPE_2 {
            return vk::SampleCountFlags::TYPE_2;
        }

        return vk::SampleCountFlags::TYPE_1;
    }

    #[must_use]
    pub fn create_image_ex(
        &mut self,
        width: u32,
        height: u32,
        depth: u32,
        mip_map_level_count: usize,
        format: vk::Format,
        tiling: vk::ImageTiling,
        image: &mut vk::Image,
        image_memory: &mut SMemoryImageBlock<IMAGE_BUFFER_CACHE_ID>,
        image_usage: vk::ImageUsageFlags,
        sample_count: Option<u32>,
    ) -> bool {
        let mut image_info = vk::ImageCreateInfo::default();
        image_info.image_type = vk::ImageType::TYPE_2D;
        image_info.extent.width = width;
        image_info.extent.height = height;
        image_info.extent.depth = 1;
        image_info.mip_levels = mip_map_level_count as u32;
        image_info.array_layers = depth;
        image_info.format = format;
        image_info.tiling = tiling;
        image_info.initial_layout = vk::ImageLayout::UNDEFINED;
        image_info.usage = image_usage;
        image_info.samples = if let Some(sample_count) = sample_count {
            Self::get_sample_count(sample_count, &self.limits)
        } else {
            vk::SampleCountFlags::TYPE_1
        };
        image_info.sharing_mode = vk::SharingMode::EXCLUSIVE;

        let res = unsafe { self.device.create_image(&image_info, None) };
        if res.is_err() {
            // TODO dbg_msg("vulkan", "failed to create image!");
        }
        *image = res.unwrap();

        let mem_requirements = unsafe { self.device.get_image_memory_requirements(*image) };

        if !self.get_image_memory(
            image_memory,
            mem_requirements.size,
            mem_requirements.alignment,
            mem_requirements.memory_type_bits,
        ) {
            return false;
        }

        unsafe {
            self.device.bind_image_memory(
                *image,
                image_memory.base.buffer_mem.mem,
                image_memory.base.heap_data.offset_to_align as u64,
            );
        }

        return true;
    }

    #[must_use]
    pub fn create_image(
        &mut self,
        width: u32,
        height: u32,
        depth: u32,
        mip_map_level_count: usize,
        format: vk::Format,
        tiling: vk::ImageTiling,
        image: &mut vk::Image,
        image_memory: &mut SMemoryImageBlock<IMAGE_BUFFER_CACHE_ID>,
    ) -> bool {
        return self.create_image_ex(
            width,
            height,
            depth,
            mip_map_level_count,
            format,
            tiling,
            image,
            image_memory,
            vk::ImageUsageFlags::TRANSFER_SRC
                | vk::ImageUsageFlags::TRANSFER_DST
                | vk::ImageUsageFlags::SAMPLED,
            None,
        );
    }

    #[must_use]
    pub fn image_barrier(
        &mut self,
        image: vk::Image,
        mip_map_base: usize,
        mip_map_count: usize,
        layer_base: usize,
        layer_count: usize,
        _format: vk::Format,
        old_layout: vk::ImageLayout,
        new_layout: vk::ImageLayout,
        cur_image_index: u32,
    ) -> bool {
        let mut ptr_mem_command_buffer: *mut vk::CommandBuffer = std::ptr::null_mut();
        if !self.get_memory_command_buffer(&mut ptr_mem_command_buffer, cur_image_index) {
            return false;
        }
        let mem_command_buffer = unsafe { &mut *ptr_mem_command_buffer };

        let mut barrier = vk::ImageMemoryBarrier::default();
        barrier.old_layout = old_layout;
        barrier.new_layout = new_layout;
        barrier.src_queue_family_index = vk::QUEUE_FAMILY_IGNORED;
        barrier.dst_queue_family_index = vk::QUEUE_FAMILY_IGNORED;
        barrier.image = image;
        barrier.subresource_range.aspect_mask = vk::ImageAspectFlags::COLOR;
        barrier.subresource_range.base_mip_level = mip_map_base as u32;
        barrier.subresource_range.level_count = mip_map_count as u32;
        barrier.subresource_range.base_array_layer = layer_base as u32;
        barrier.subresource_range.layer_count = layer_count as u32;

        let mut source_stage = vk::PipelineStageFlags::TOP_OF_PIPE;
        let mut destination_stage = vk::PipelineStageFlags::TRANSFER;

        if old_layout == vk::ImageLayout::UNDEFINED
            && new_layout == vk::ImageLayout::TRANSFER_DST_OPTIMAL
        {
            barrier.src_access_mask = vk::AccessFlags::empty();
            barrier.dst_access_mask = vk::AccessFlags::TRANSFER_WRITE;

            source_stage = vk::PipelineStageFlags::TOP_OF_PIPE;
            destination_stage = vk::PipelineStageFlags::TRANSFER;
        } else if old_layout == vk::ImageLayout::TRANSFER_DST_OPTIMAL
            && new_layout == vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL
        {
            barrier.src_access_mask = vk::AccessFlags::TRANSFER_WRITE;
            barrier.dst_access_mask = vk::AccessFlags::SHADER_READ;

            source_stage = vk::PipelineStageFlags::TRANSFER;
            destination_stage = vk::PipelineStageFlags::FRAGMENT_SHADER;
        } else if old_layout == vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL
            && new_layout == vk::ImageLayout::TRANSFER_DST_OPTIMAL
        {
            barrier.src_access_mask = vk::AccessFlags::SHADER_READ;
            barrier.dst_access_mask = vk::AccessFlags::TRANSFER_WRITE;

            source_stage = vk::PipelineStageFlags::FRAGMENT_SHADER;
            destination_stage = vk::PipelineStageFlags::TRANSFER;
        } else if old_layout == vk::ImageLayout::TRANSFER_SRC_OPTIMAL
            && new_layout == vk::ImageLayout::PRESENT_SRC_KHR
        {
            barrier.src_access_mask = vk::AccessFlags::TRANSFER_READ;
            barrier.dst_access_mask = vk::AccessFlags::MEMORY_READ;

            source_stage = vk::PipelineStageFlags::TRANSFER;
            destination_stage = vk::PipelineStageFlags::BOTTOM_OF_PIPE;
        } else if old_layout == vk::ImageLayout::PRESENT_SRC_KHR
            && new_layout == vk::ImageLayout::TRANSFER_SRC_OPTIMAL
        {
            barrier.src_access_mask = vk::AccessFlags::MEMORY_READ;
            barrier.dst_access_mask = vk::AccessFlags::TRANSFER_READ;

            source_stage = vk::PipelineStageFlags::BOTTOM_OF_PIPE;
            destination_stage = vk::PipelineStageFlags::TRANSFER;
        } else if old_layout == vk::ImageLayout::UNDEFINED && new_layout == vk::ImageLayout::GENERAL
        {
            barrier.src_access_mask = vk::AccessFlags::empty();
            barrier.dst_access_mask = vk::AccessFlags::MEMORY_READ;

            source_stage = vk::PipelineStageFlags::TOP_OF_PIPE;
            destination_stage = vk::PipelineStageFlags::TRANSFER;
        } else if old_layout == vk::ImageLayout::GENERAL
            && new_layout == vk::ImageLayout::TRANSFER_DST_OPTIMAL
        {
            barrier.src_access_mask = vk::AccessFlags::MEMORY_READ;
            barrier.dst_access_mask = vk::AccessFlags::TRANSFER_WRITE;

            source_stage = vk::PipelineStageFlags::TRANSFER;
            destination_stage = vk::PipelineStageFlags::TRANSFER;
        } else if old_layout == vk::ImageLayout::TRANSFER_DST_OPTIMAL
            && new_layout == vk::ImageLayout::GENERAL
        {
            barrier.src_access_mask = vk::AccessFlags::TRANSFER_WRITE;
            barrier.dst_access_mask = vk::AccessFlags::MEMORY_READ;

            source_stage = vk::PipelineStageFlags::TRANSFER;
            destination_stage = vk::PipelineStageFlags::TRANSFER;
        } else {
            // TODO dbg_msg("vulkan", "unsupported layout transition!");
        }

        unsafe {
            self.device.cmd_pipeline_barrier(
                *mem_command_buffer,
                source_stage,
                destination_stage,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                &[barrier],
            );
        }

        return true;
    }

    #[must_use]
    pub fn copy_buffer_to_image(
        &mut self,
        buffer: vk::Buffer,
        buffer_offset: vk::DeviceSize,
        image: vk::Image,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
        depth: usize,
        cur_image_index: u32,
    ) -> bool {
        let mut command_buffer_ptr: *mut vk::CommandBuffer = std::ptr::null_mut();
        if !self.get_memory_command_buffer(&mut command_buffer_ptr, cur_image_index) {
            return false;
        }
        let command_buffer = unsafe { &mut *command_buffer_ptr };

        let mut region = vk::BufferImageCopy::default();
        region.buffer_offset = buffer_offset;
        region.buffer_row_length = 0;
        region.buffer_image_height = 0;
        region.image_subresource.aspect_mask = vk::ImageAspectFlags::COLOR;
        region.image_subresource.mip_level = 0;
        region.image_subresource.base_array_layer = 0;
        region.image_subresource.layer_count = depth as u32;
        region.image_offset = vk::Offset3D { x, y, z: 0 };
        region.image_extent = vk::Extent3D {
            width,
            height,
            depth: 1,
        };

        unsafe {
            self.device.cmd_copy_buffer_to_image(
                *command_buffer,
                buffer,
                image,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                &[region],
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
        const INSTANCE_TYPE_COUNT: usize,
        const BUFFER_CREATE_COUNT: usize,
        const USES_CURRENT_COUNT_OFFSET: bool,
    >(
        mem: &Memory,
        device: &ash::Device,
        ptr_buffer_mem: &mut *mut TStreamMemName,
        new_mem_func: &'a mut dyn FnMut(&mut TStreamMemName, vk::Buffer, vk::DeviceSize) -> bool,
        stream_uniform_buffer: &mut SStreamMemory<TStreamMemName>,
        usage: vk::BufferUsageFlags,
        data_size: usize,
        cur_image_index: u32,

        buffer: &mut vk::Buffer,
        buffer_mem: &mut SDeviceMemoryBlock,
        offset: &mut usize,

        ptr_mem: &mut *mut u8,
    ) -> bool
    where
        TStreamMemName: Default + StreamMemory,
    {
        let mut it: usize = 0;
        if USES_CURRENT_COUNT_OFFSET {
            it = stream_uniform_buffer.get_used_count(cur_image_index as usize);
        }
        while it
            < stream_uniform_buffer
                .get_buffers(cur_image_index as usize)
                .len()
        {
            let mut buffer_of_frame =
                &mut stream_uniform_buffer.get_buffers(cur_image_index as usize)[it];
            if *buffer_of_frame.get_size() >= data_size + *buffer_of_frame.get_used_size() {
                if !*buffer_of_frame.get_is_used() {
                    buffer_of_frame.set_is_used();
                    stream_uniform_buffer.increase_used_count(cur_image_index as usize);
                    buffer_of_frame =
                        &mut stream_uniform_buffer.get_buffers(cur_image_index as usize)[it];
                }
                *buffer = *buffer_of_frame.get_buffer();
                *buffer_mem = buffer_of_frame.get_device_mem_block().clone();
                *offset = *buffer_of_frame.get_used_size();
                *buffer_of_frame.get_used_size() += data_size;
                *ptr_mem = *buffer_of_frame.get_mapped_buffer_data() as *mut u8;
                *ptr_buffer_mem = buffer_of_frame;
                break;
            }
            it += 1;
        }

        if buffer_mem.mem == vk::DeviceMemory::null() {
            // create memory
            let mut stream_buffer = vk::Buffer::null();
            let mut stream_buffer_memory = SDeviceMemoryBlock::default();
            let new_buffer_single_size =
                (std::mem::size_of::<TInstanceTypeName>() * INSTANCE_TYPE_COUNT) as vk::DeviceSize;
            let new_buffer_size =
                (new_buffer_single_size * BUFFER_CREATE_COUNT as u64) as vk::DeviceSize;
            if !mem.create_buffer(
                new_buffer_size,
                EMemoryBlockUsage::Stream,
                usage,
                vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_CACHED,
                &mut stream_buffer,
                &mut stream_buffer_memory,
            ) {
                return false;
            }

            let ptr_mapped_data = unsafe {
                device.map_memory(
                    stream_buffer_memory.mem,
                    0,
                    vk::WHOLE_SIZE,
                    vk::MemoryMapFlags::empty(),
                )
            }
            .unwrap();

            let new_buffer_index: usize = stream_uniform_buffer
                .get_buffers(cur_image_index as usize)
                .len();
            for i in 0..BUFFER_CREATE_COUNT {
                stream_uniform_buffer
                    .get_buffers(cur_image_index as usize)
                    .push(TStreamMemName::new(
                        stream_buffer,
                        stream_buffer_memory.clone(),
                        new_buffer_single_size as usize * i,
                        new_buffer_single_size as usize,
                        0,
                        unsafe {
                            (ptr_mapped_data as *mut u8)
                                .offset((new_buffer_single_size as isize * i as isize) as isize)
                        } as *mut c_void,
                    ));
                stream_uniform_buffer
                    .get_ranges(cur_image_index as usize)
                    .push(Default::default());
                if !new_mem_func(
                    stream_uniform_buffer
                        .get_buffers(cur_image_index as usize)
                        .last_mut()
                        .unwrap(),
                    stream_buffer,
                    new_buffer_single_size * i as u64,
                ) {
                    return false;
                }
            }
            let new_stream_buffer =
                &mut stream_uniform_buffer.get_buffers(cur_image_index as usize)[new_buffer_index];

            *buffer = stream_buffer;
            *buffer_mem = stream_buffer_memory;

            *ptr_buffer_mem = new_stream_buffer;
            *ptr_mem = *new_stream_buffer.get_mapped_buffer_data() as *mut u8;
            *offset = *new_stream_buffer.get_offset_in_buffer();
            *new_stream_buffer.get_used_size() += data_size;
            new_stream_buffer.set_is_used();

            stream_uniform_buffer.increase_used_count(cur_image_index as usize);
        }
        return true;
    }

    // returns true, if the stream memory was just allocated
    #[must_use]
    pub fn create_stream_buffer<
        'a,
        TStreamMemName: Clone,
        TInstanceTypeName,
        const INSTANCE_TYPE_COUNT: usize,
        const BUFFER_CREATE_COUNT: usize,
        const USES_CURRENT_COUNT_OFFSET: bool,
    >(
        mem: &Memory,
        device: &ash::Device,
        ptr_buffer_mem: &mut *mut TStreamMemName,
        new_mem_func: &'a mut dyn FnMut(&mut TStreamMemName, vk::Buffer, vk::DeviceSize) -> bool,
        stream_uniform_buffer: &mut SStreamMemory<TStreamMemName>,
        usage: vk::BufferUsageFlags,
        new_buffer: &mut vk::Buffer,
        new_buffer_mem: &mut SDeviceMemoryBlock,
        buffer_offset: &mut usize,
        ptr_raw_data: *const c_void,
        data_size: usize,
        cur_image_index: u32,
    ) -> bool
    where
        TStreamMemName: Default + StreamMemory,
    {
        let mut buffer = vk::Buffer::null();
        let mut buffer_mem = SDeviceMemoryBlock::default();
        let mut offset: usize = 0;

        let mut ptr_mem: *mut u8 = std::ptr::null_mut();

        Self::create_stream_buffer_unallocated::<
            TStreamMemName,
            TInstanceTypeName,
            INSTANCE_TYPE_COUNT,
            BUFFER_CREATE_COUNT,
            USES_CURRENT_COUNT_OFFSET,
        >(
            mem,
            device,
            ptr_buffer_mem,
            new_mem_func,
            stream_uniform_buffer,
            usage,
            data_size,
            cur_image_index,
            &mut buffer,
            &mut buffer_mem,
            &mut offset,
            &mut ptr_mem,
        );

        unsafe {
            libc::memcpy(
                ptr_mem.offset(offset as isize) as *mut c_void,
                ptr_raw_data,
                data_size as usize,
            );
        }

        *new_buffer = buffer;
        *new_buffer_mem = buffer_mem;
        *buffer_offset = offset;

        return true;
    }

    #[must_use]
    pub fn get_uniform_buffer_object_impl<
        TName,
        const INSTANCE_MAX_PARTICLE_COUNT: usize,
        const MAX_INSTANCES: usize,
    >(
        &mut self,
        render_thread_index: usize,
        requires_shared_stages_descriptor: bool,
        descr_set: &mut SDeviceDescriptorSet,
        ptr_raw_data: *const c_void,
        data_size: usize,
        cur_image_index: u32,
    ) -> bool {
        let mut new_buffer = vk::Buffer::null();
        let mut new_buffer_mem = SDeviceMemoryBlock::default();
        let mut buffer_offset = usize::default();
        let mut ptr_mem: *mut SFrameUniformBuffers = std::ptr::null_mut();
        let mem = &self.mem;
        let device = &self.device;
        let error = &self.error;
        let pools = &mut self.uniform_buffer_descr_pools[render_thread_index];
        let sprite_descr_layout = &self.sprite_multi_uniform_descriptor_set_layout;
        let quad_descr_layout = &self.quad_uniform_descriptor_set_layout;
        let stream_uniform_buffer = &mut self.streamed_uniform_buffers[render_thread_index];
        let mut new_mem_func = move |mem: &mut SFrameUniformBuffers,
                                     buffer: vk::Buffer,
                                     mem_offset: vk::DeviceSize|
              -> bool {
            if !Self::create_uniform_descriptor_sets(
                error,
                device,
                pools,
                sprite_descr_layout,
                mem.uniform_sets.as_mut_ptr(),
                1,
                buffer,
                INSTANCE_MAX_PARTICLE_COUNT * std::mem::size_of::<TName>(),
                mem_offset,
            ) {
                return false;
            }
            if !Self::create_uniform_descriptor_sets(
                error,
                device,
                pools,
                quad_descr_layout,
                unsafe { &mut (*(mem as *mut _ as *mut SFrameUniformBuffers)).uniform_sets[1] },
                1,
                buffer,
                INSTANCE_MAX_PARTICLE_COUNT * std::mem::size_of::<TName>(),
                mem_offset,
            ) {
                return false;
            }
            return true;
        };
        if !Self::create_stream_buffer::<
            SFrameUniformBuffers,
            TName,
            INSTANCE_MAX_PARTICLE_COUNT,
            MAX_INSTANCES,
            true,
        >(
            mem,
            device,
            &mut ptr_mem,
            &mut new_mem_func,
            stream_uniform_buffer,
            vk::BufferUsageFlags::UNIFORM_BUFFER,
            &mut new_buffer,
            &mut new_buffer_mem,
            &mut buffer_offset,
            ptr_raw_data,
            data_size,
            cur_image_index,
        ) {
            return false;
        }

        *descr_set = unsafe { &mut *ptr_mem }.uniform_sets[if requires_shared_stages_descriptor {
            1
        } else {
            0
        }]
        .clone();
        return true;
    }

    #[must_use]
    pub fn create_index_buffer(
        &mut self,
        ptr_raw_data: *const c_void,
        data_size: usize,
        buffer: &mut vk::Buffer,
        memory: &mut SDeviceMemoryBlock,
        cur_image_index: u32,
    ) -> bool {
        let buffer_data_size = data_size as vk::DeviceSize;

        let mut staging_buffer = SMemoryBlock::<STAGING_BUFFER_CACHE_ID>::default();
        if !self.get_staging_buffer(&mut staging_buffer, ptr_raw_data, data_size as u64) {
            return false;
        }

        let mut vertex_buffer_memory = SDeviceMemoryBlock::default();
        let mut vertex_buffer = vk::Buffer::null();
        if !self.mem.create_buffer(
            buffer_data_size,
            EMemoryBlockUsage::Buffer,
            vk::BufferUsageFlags::TRANSFER_DST | vk::BufferUsageFlags::INDEX_BUFFER,
            vk::MemoryPropertyFlags::DEVICE_LOCAL,
            &mut vertex_buffer,
            &mut vertex_buffer_memory,
        ) {
            return false;
        }

        if !self.memory_barrier(
            vertex_buffer,
            0,
            buffer_data_size,
            vk::AccessFlags::INDEX_READ,
            true,
            cur_image_index,
        ) {
            return false;
        }
        if !self.copy_buffer(
            staging_buffer.buffer,
            vertex_buffer,
            staging_buffer.heap_data.offset_to_align as u64,
            0,
            buffer_data_size,
            cur_image_index,
        ) {
            return false;
        }
        if !self.memory_barrier(
            vertex_buffer,
            0,
            buffer_data_size,
            vk::AccessFlags::INDEX_READ,
            false,
            cur_image_index,
        ) {
            return false;
        }

        self.upload_and_free_staging_mem_block(&mut staging_buffer, cur_image_index);

        *buffer = vertex_buffer;
        *memory = vertex_buffer_memory;
        return true;
    }

    pub fn destroy_index_buffer(
        &mut self,
        buffer: &mut vk::Buffer,
        memory: &mut SDeviceMemoryBlock,
    ) {
        self.mem.clean_buffer_pair(0, buffer, memory);
    }

    /************************
     * BUFFERS
     ************************/
    #[must_use]
    pub fn create_stream_vertex_buffer(
        &mut self,
        new_buffer: &mut vk::Buffer,
        new_buffer_mem: &mut SDeviceMemoryBlock,
        buffer_offset: &mut usize,
        ptr_raw_data: &mut *mut u8,
        data_size: usize,
        cur_image_index: u32,
    ) -> bool {
        let mut ptr_stream_buffer: *mut SFrameBuffers = std::ptr::null_mut();
        return Self::create_stream_buffer_unallocated::<
            SFrameBuffers,
            GlVertexTex3DStream,
            { StreamDataMax::MaxVertices as usize * 2 },
            1,
            false,
        >(
            &self.mem,
            &self.device,
            &mut ptr_stream_buffer,
            &mut |_: &mut SFrameBuffers, _: vk::Buffer, _: vk::DeviceSize| -> bool {
                return true;
            },
            &mut self.streamed_vertex_buffer,
            vk::BufferUsageFlags::VERTEX_BUFFER,
            data_size,
            cur_image_index,
            new_buffer,
            new_buffer_mem,
            buffer_offset,
            ptr_raw_data,
        );
    }

    #[must_use]
    pub fn update_stream_vertex_buffer(
        &mut self,
        block_size: usize,
        data_size: usize,
        cur_image_index: u32,
    ) {
        if !self
            .streamed_vertex_buffer
            .get_buffers(cur_image_index as usize)
            .is_empty()
        {
            let cur_buffer = self
                .streamed_vertex_buffer
                .get_current_buffer(cur_image_index as usize);

            // remove the size of the block (which is the allocated size)
            cur_buffer.used_size -= block_size;
            // only add what was actually used
            cur_buffer.used_size += data_size;
        }
    }

    #[must_use]
    pub fn create_buffer_object(
        &mut self,
        buffer_index: usize,
        mut upload_data: GraphicsBackendMemory,
        buffer_data_size: vk::DeviceSize,
        cur_image_index: u32,
    ) -> bool {
        while buffer_index >= self.buffer_objects.len() {
            self.buffer_objects.resize(
                (self.buffer_objects.len() * 2) + 1,
                SBufferObjectFrame::default(),
            );
        }

        let tmp_allocator = self.mem_allocator.clone();
        let mut mem_allocator = tmp_allocator.lock().unwrap();
        let allocator = mem_allocator.as_mut().unwrap();
        let staging_buffer = allocator
            .get_mem_block(upload_data.as_mut_slice().as_ptr() as *mut c_void)
            .unwrap();

        let mut mem = SMemoryBlock::<VERTEX_BUFFER_CACHE_ID>::default();
        if !self.get_vertex_buffer(&mut mem, buffer_data_size) {
            return false;
        }

        let buffer_object = &mut self.buffer_objects[buffer_index];
        buffer_object.buffer_object.mem = mem.clone();
        let vertex_buffer = mem.buffer;
        let buffer_offset = mem.heap_data.offset_to_align;

        if !self.memory_barrier(
            vertex_buffer,
            mem.heap_data.offset_to_align as u64,
            buffer_data_size,
            vk::AccessFlags::VERTEX_ATTRIBUTE_READ,
            true,
            cur_image_index,
        ) {
            return false;
        }
        if !self.copy_buffer(
            staging_buffer.buffer,
            vertex_buffer,
            staging_buffer.heap_data.offset_to_align as u64,
            mem.heap_data.offset_to_align as u64,
            buffer_data_size,
            cur_image_index,
        ) {
            return false;
        }
        if !self.memory_barrier(
            vertex_buffer,
            mem.heap_data.offset_to_align as u64,
            buffer_data_size,
            vk::AccessFlags::VERTEX_ATTRIBUTE_READ,
            false,
            cur_image_index,
        ) {
            return false;
        }
        allocator.upload_and_free_mem(upload_data, cur_image_index, |block, queue_el| {
            self.prepare_staging_mem_range_impl(block, queue_el);
        });

        let buffer_object = &mut self.buffer_objects[buffer_index];
        buffer_object.cur_buffer = vertex_buffer;
        buffer_object.cur_buffer_offset = buffer_offset;

        return true;
    }

    pub fn delete_buffer_object(&mut self, buffer_index: usize, cur_image_index: u32) {
        let mut delete_obj: SBufferObjectFrame = Default::default();
        std::mem::swap(&mut delete_obj, &mut self.buffer_objects[buffer_index]);
        Self::free_vertex_mem_block(
            &mut self.frame_delayed_buffer_cleanups,
            &mut self.vertex_buffer_cache,
            &delete_obj.buffer_object.mem,
            cur_image_index,
        );
    }

    #[must_use]
    pub fn copy_buffer(
        &mut self,
        src_buffer: vk::Buffer,
        dst_buffer: vk::Buffer,
        src_offset: vk::DeviceSize,
        dst_offset: vk::DeviceSize,
        copy_size: vk::DeviceSize,
        cur_image_index: u32,
    ) -> bool {
        let mut command_buffer_ptr: *mut vk::CommandBuffer = std::ptr::null_mut();
        if !self.get_memory_command_buffer(&mut command_buffer_ptr, cur_image_index) {
            return false;
        }
        let command_buffer = unsafe { &mut *command_buffer_ptr };
        let mut copy_region = vk::BufferCopy::default();
        copy_region.src_offset = src_offset;
        copy_region.dst_offset = dst_offset;
        copy_region.size = copy_size;
        unsafe {
            self.device
                .cmd_copy_buffer(*command_buffer, src_buffer, dst_buffer, &[copy_region]);
        }

        return true;
    }

    #[must_use]
    pub fn allocate_descriptor_pool(
        error: &Arc<Mutex<Error>>,
        device: &ash::Device,
        descriptor_pools: &mut SDeviceDescriptorPools,
        alloc_pool_size: usize,
    ) -> bool {
        let mut new_pool = SDeviceDescriptorPool::default();
        new_pool.size = alloc_pool_size as u64;

        let mut pool_size = vk::DescriptorPoolSize::default();
        if descriptor_pools.is_uniform_pool {
            pool_size.ty = vk::DescriptorType::UNIFORM_BUFFER;
        } else {
            pool_size.ty = vk::DescriptorType::COMBINED_IMAGE_SAMPLER;
        }
        pool_size.descriptor_count = alloc_pool_size as u32;

        let mut pool_info = vk::DescriptorPoolCreateInfo::default();
        pool_info.pool_size_count = 1;
        pool_info.p_pool_sizes = &pool_size;
        pool_info.max_sets = alloc_pool_size as u32;
        pool_info.flags = vk::DescriptorPoolCreateFlags::FREE_DESCRIPTOR_SET;

        let res = unsafe { device.create_descriptor_pool(&pool_info, None) };
        if res.is_err() {
            error.lock().unwrap().set_error(
                EGFXErrorType::Init,
                localizable("Creating the descriptor pool failed."),
            );
            return false;
        }
        new_pool.pool = res.unwrap();

        descriptor_pools.pools.push(new_pool);

        return true;
    }

    #[must_use]
    pub fn get_descriptor_pool_for_alloc(
        error: &Arc<Mutex<Error>>,
        device: &ash::Device,
        ret_descr: &mut vk::DescriptorPool,
        descriptor_pools: &mut SDeviceDescriptorPools,
        ptr_sets: *mut SDeviceDescriptorSet,
        alloc_num: usize,
    ) -> bool {
        let mut cur_alloc_num = alloc_num;
        let mut cur_alloc_offset = 0;
        *ret_descr = vk::DescriptorPool::null();

        while cur_alloc_num > 0 {
            let mut allocated_in_this_run = 0;

            let mut found = false;
            let mut descriptor_pool_index = usize::MAX;
            for i in 0..descriptor_pools.pools.len() {
                let pool = &mut descriptor_pools.pools[i];
                if pool.cur_size + (cur_alloc_num as u64) < pool.size {
                    allocated_in_this_run = cur_alloc_num;
                    pool.cur_size += cur_alloc_num as u64;
                    found = true;
                    if *ret_descr == vk::DescriptorPool::null() {
                        *ret_descr = pool.pool;
                    }
                    descriptor_pool_index = i;
                    break;
                } else {
                    let remaining_pool_count = pool.size - pool.cur_size;
                    if remaining_pool_count > 0 {
                        allocated_in_this_run = remaining_pool_count as usize;
                        pool.cur_size += remaining_pool_count;
                        found = true;
                        if *ret_descr == vk::DescriptorPool::null() {
                            *ret_descr = pool.pool;
                        }
                        descriptor_pool_index = i;
                        break;
                    }
                }
            }

            if !found {
                descriptor_pool_index = descriptor_pools.pools.len();

                if !Self::allocate_descriptor_pool(
                    error,
                    device,
                    descriptor_pools,
                    descriptor_pools.default_alloc_size as usize,
                ) {
                    return false;
                }

                allocated_in_this_run =
                    std::cmp::min(descriptor_pools.default_alloc_size as usize, cur_alloc_num);

                let pool = descriptor_pools.pools.last_mut().unwrap();
                pool.cur_size += allocated_in_this_run as u64;
                if *ret_descr == vk::DescriptorPool::null() {
                    *ret_descr = pool.pool;
                }
            }

            for i in cur_alloc_offset..cur_alloc_offset + allocated_in_this_run {
                unsafe {
                    (*ptr_sets.offset(i as isize)).pools = descriptor_pools;
                    (*ptr_sets.offset(i as isize)).pool_index = descriptor_pool_index;
                }
            }
            cur_alloc_offset += allocated_in_this_run;
            cur_alloc_num -= allocated_in_this_run;
        }

        return true;
    }

    #[must_use]
    pub fn create_new_textured_descriptor_sets_impl(
        &mut self,
        descr_index: usize,
        texture: &mut CTexture,
    ) -> bool {
        let descr_set = &mut texture.vk_standard_textured_descr_sets[descr_index];

        let descr_pool = &mut self.standard_texture_descr_pool;

        let mut des_alloc_info = vk::DescriptorSetAllocateInfo::default();
        if !Self::get_descriptor_pool_for_alloc(
            &self.error,
            &self.device,
            &mut des_alloc_info.descriptor_pool,
            descr_pool,
            descr_set,
            1,
        ) {
            return false;
        }
        des_alloc_info.descriptor_set_count = 1;
        des_alloc_info.p_set_layouts = &self.standard_textured_descriptor_set_layout;

        let res = unsafe { self.device.allocate_descriptor_sets(&des_alloc_info) };
        if res.is_err() {
            return false;
        }
        descr_set.descriptor = res.unwrap()[0]; // TODO: array access

        let mut image_info = vk::DescriptorImageInfo::default();
        image_info.image_layout = vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL;
        image_info.image_view = texture.img_view;
        image_info.sampler = texture.samplers[descr_index];

        let mut descriptor_writes: [vk::WriteDescriptorSet; 1] = Default::default();
        descriptor_writes[0].dst_set = descr_set.descriptor;
        descriptor_writes[0].dst_binding = 0;
        descriptor_writes[0].dst_array_element = 0;
        descriptor_writes[0].descriptor_type = vk::DescriptorType::COMBINED_IMAGE_SAMPLER;
        descriptor_writes[0].descriptor_count = 1;
        descriptor_writes[0].p_image_info = &image_info;

        unsafe {
            self.device
                .update_descriptor_sets(descriptor_writes.as_slice(), &[]);
        }

        return true;
    }

    #[must_use]
    pub fn create_new_textured_standard_descriptor_sets(
        &mut self,
        descr_index: usize,
        texture: &mut CTexture,
    ) -> bool {
        self.create_new_textured_descriptor_sets_impl(descr_index, texture)
    }

    pub fn destroy_textured_standard_descriptor_sets(
        device: &ash::Device,
        texture: &mut CTexture,
        descr_index: usize,
    ) {
        let descr_set = &mut texture.vk_standard_textured_descr_sets[descr_index];
        if descr_set.pool_index != usize::MAX {
            unsafe {
                device.free_descriptor_sets(
                    (*descr_set.pools).pools[descr_set.pool_index].pool,
                    &[descr_set.descriptor],
                );
            }
        }
        *descr_set = Default::default();
    }

    #[must_use]
    pub fn create_new_3d_textured_standard_descriptor_sets(
        &mut self,
        _texture_slot: usize,
        texture: &mut CTexture,
    ) -> bool {
        let descr_set = &mut texture.vk_standard_3d_textured_descr_set;

        let mut des_alloc_info = vk::DescriptorSetAllocateInfo::default();
        if !Self::get_descriptor_pool_for_alloc(
            &self.error,
            &self.device,
            &mut des_alloc_info.descriptor_pool,
            &mut self.standard_texture_descr_pool,
            descr_set,
            1,
        ) {
            return false;
        }
        des_alloc_info.descriptor_set_count = 1;
        des_alloc_info.p_set_layouts = &mut self.standard_3d_textured_descriptor_set_layout;

        let res = unsafe { self.device.allocate_descriptor_sets(&des_alloc_info) };
        if res.is_err() {
            return false;
        }
        descr_set.descriptor = res.unwrap()[0]; // TODO array access

        let mut image_info = vk::DescriptorImageInfo::default();
        image_info.image_layout = vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL;
        image_info.image_view = texture.img_3d_view;
        image_info.sampler = texture.sampler_3d;

        let mut descriptor_writes: [vk::WriteDescriptorSet; 1] = Default::default();

        descriptor_writes[0].dst_set = descr_set.descriptor;
        descriptor_writes[0].dst_binding = 0;
        descriptor_writes[0].dst_array_element = 0;
        descriptor_writes[0].descriptor_type = vk::DescriptorType::COMBINED_IMAGE_SAMPLER;
        descriptor_writes[0].descriptor_count = 1;
        descriptor_writes[0].p_image_info = &image_info;

        unsafe {
            self.device
                .update_descriptor_sets(descriptor_writes.as_slice(), &[]);
        }

        return true;
    }

    pub fn destroy_textured_3d_standard_descriptor_sets(
        device: &ash::Device,
        texture: &mut CTexture,
    ) {
        let descr_set = &mut texture.vk_standard_3d_textured_descr_set;
        if descr_set.pool_index != usize::MAX {
            unsafe {
                device.free_descriptor_sets(
                    (*descr_set.pools).pools[descr_set.pool_index].pool,
                    &[descr_set.descriptor],
                );
            }
        }
    }

    #[must_use]
    pub fn create_new_text_descriptor_sets(
        &mut self,
        texture: usize,
        texture_outline: usize,
    ) -> bool {
        let texture_text = &mut self.textures[texture];
        let descr_set_text = &mut texture_text.vk_text_descr_set;

        let mut des_alloc_info = vk::DescriptorSetAllocateInfo::default();
        if !Self::get_descriptor_pool_for_alloc(
            &self.error,
            &self.device,
            &mut des_alloc_info.descriptor_pool,
            &mut self.text_texture_descr_pool,
            descr_set_text,
            1,
        ) {
            return false;
        }
        des_alloc_info.descriptor_set_count = 1;
        des_alloc_info.p_set_layouts = &self.text_descriptor_set_layout;

        let res = unsafe { self.device.allocate_descriptor_sets(&des_alloc_info) };
        if res.is_err() {
            return false;
        }

        descr_set_text.descriptor = res.unwrap()[0];
        let mut descriptor_writes: [vk::WriteDescriptorSet; 2] = Default::default();
        descriptor_writes[0].dst_set = descr_set_text.descriptor;

        let mut image_info: [vk::DescriptorImageInfo; 2] = Default::default();
        image_info[0].image_layout = vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL;
        image_info[0].image_view = texture_text.img_view;
        image_info[0].sampler = texture_text.samplers[0];
        image_info[1].image_layout = vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL;
        let texture_text_outline = &mut self.textures[texture_outline];
        image_info[1].image_view = texture_text_outline.img_view;
        image_info[1].sampler = texture_text_outline.samplers[0];

        descriptor_writes[0].dst_binding = 0;
        descriptor_writes[0].dst_array_element = 0;
        descriptor_writes[0].descriptor_type = vk::DescriptorType::COMBINED_IMAGE_SAMPLER;
        descriptor_writes[0].descriptor_count = 1;
        descriptor_writes[0].p_image_info = image_info.as_ptr();
        descriptor_writes[1] = descriptor_writes[0];
        descriptor_writes[1].dst_binding = 1;
        descriptor_writes[1].p_image_info = &image_info[1];

        unsafe {
            self.device
                .update_descriptor_sets(descriptor_writes.as_slice(), &[]);
        }

        return true;
    }

    pub fn destroy_text_descriptor_sets(
        device: &ash::Device,
        texture: &mut CTexture,
        _texture_outline: &mut CTexture,
    ) {
        let descr_set = &mut texture.vk_text_descr_set;
        if descr_set.pool_index != usize::MAX {
            unsafe {
                device.free_descriptor_sets(
                    (*descr_set.pools).pools[descr_set.pool_index].pool,
                    &[descr_set.descriptor],
                );
            }
        }
    }

    #[must_use]
    pub fn create_uniform_descriptor_set_layout(
        error: &Arc<Mutex<Error>>,
        device: &ash::Device,
        set_layout: &mut vk::DescriptorSetLayout,
        stage_flags: vk::ShaderStageFlags,
    ) -> bool {
        let mut sampler_layout_binding = vk::DescriptorSetLayoutBinding::default();
        sampler_layout_binding.binding = 1;
        sampler_layout_binding.descriptor_count = 1;
        sampler_layout_binding.descriptor_type = vk::DescriptorType::UNIFORM_BUFFER;
        sampler_layout_binding.p_immutable_samplers = std::ptr::null();
        sampler_layout_binding.stage_flags = stage_flags;

        let bindings = [sampler_layout_binding];
        let mut layout_info = vk::DescriptorSetLayoutCreateInfo::default();
        layout_info.binding_count = bindings.len() as u32;
        layout_info.p_bindings = bindings.as_ptr();

        let res = unsafe { device.create_descriptor_set_layout(&layout_info, None) };
        if res.is_err() {
            error.lock().unwrap().set_error(
                EGFXErrorType::Init,
                localizable("Creating descriptor layout failed."),
            );
            return false;
        }
        *set_layout = res.unwrap();
        return true;
    }

    #[must_use]
    pub fn create_sprite_multi_uniform_descriptor_set_layout(&mut self) -> bool {
        return Self::create_uniform_descriptor_set_layout(
            &self.error,
            &self.device,
            &mut self.sprite_multi_uniform_descriptor_set_layout,
            vk::ShaderStageFlags::VERTEX,
        );
    }

    #[must_use]
    pub fn create_quad_uniform_descriptor_set_layout(&mut self) -> bool {
        return Self::create_uniform_descriptor_set_layout(
            &self.error,
            &self.device,
            &mut self.quad_uniform_descriptor_set_layout,
            vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
        );
    }

    pub fn destroy_uniform_descriptor_set_layouts(&mut self) {
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
    pub fn create_uniform_descriptor_sets(
        error: &Arc<Mutex<Error>>,
        device: &ash::Device,
        descr_pools: &mut SDeviceDescriptorPools,
        set_layout: &vk::DescriptorSetLayout,
        ptr_sets: *mut SDeviceDescriptorSet,
        set_count: usize,
        bind_buffer: vk::Buffer,
        single_buffer_instance_size: usize,
        memory_offset: vk::DeviceSize,
    ) -> bool {
        let mut ret_descr = vk::DescriptorPool::default();
        if !Self::get_descriptor_pool_for_alloc(
            error,
            device,
            &mut ret_descr,
            descr_pools,
            ptr_sets,
            set_count,
        ) {
            return false;
        }
        let mut des_alloc_info = vk::DescriptorSetAllocateInfo::default();
        des_alloc_info.descriptor_set_count = 1;
        des_alloc_info.p_set_layouts = set_layout;
        for i in 0..set_count {
            des_alloc_info.descriptor_pool = unsafe {
                (*(*ptr_sets.offset(i as isize)).pools).pools
                    [(*ptr_sets.offset(i as isize)).pool_index]
                    .pool
            };
            let res = unsafe { device.allocate_descriptor_sets(&des_alloc_info) };
            if res.is_err() {
                return false;
            }
            unsafe {
                (*ptr_sets.offset(i as isize)).descriptor = res.unwrap()[0];
            } // TODO [0] right?

            let mut buffer_info = vk::DescriptorBufferInfo::default();
            buffer_info.buffer = bind_buffer;
            buffer_info.offset = memory_offset + (single_buffer_instance_size * i) as u64;
            buffer_info.range = single_buffer_instance_size as u64;

            let mut descriptor_writes: [vk::WriteDescriptorSet; 1] = Default::default();
            descriptor_writes[0].dst_set = unsafe { (*ptr_sets.offset(i as isize)).descriptor };
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

    pub fn destroy_uniform_descriptor_sets(
        device: &ash::Device,
        ptr_sets: *mut SDeviceDescriptorSet,
        set_count: usize,
    ) {
        for i in 0..set_count {
            unsafe {
                device.free_descriptor_sets(
                    (*(*ptr_sets.offset(i as isize)).pools).pools
                        [(*ptr_sets.offset(i as isize)).pool_index]
                        .pool,
                    &[(*ptr_sets.offset(i as isize)).descriptor],
                );
            }
            unsafe {
                (*ptr_sets.offset(i as isize)).descriptor = vk::DescriptorSet::null();
            }
        }
    }

    pub fn get_texture_sampler(&self, sampler_type: ESupportedSamplerTypes) -> vk::Sampler {
        return self.samplers[sampler_type as usize];
    }
}
