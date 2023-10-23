use std::{
    collections::HashMap,
    rc::Rc,
    sync::{
        atomic::{AtomicU64, AtomicU8},
        Arc,
    },
};

use anyhow::anyhow;
use ash::vk;
use base_log::log::{SystemLog, SystemLogGroup};
use graphics_types::{
    command_buffer::{GlVertexTex3DStream, StreamDataMax},
    rendering::WRAP_TYPE_COUNT,
};
use libc::c_void;

use super::{
    barriers::{image_barrier, memory_barrier},
    buffer::Buffer,
    command_buffer::CommandBuffers,
    descriptor_layout::DescriptorSetLayout,
    descriptor_set::DescriptorSet,
    image::{GetImg, Image},
    image_view::ImageView,
    instance::Instance,
    logical_device::LogicalDevice,
    mapped_memory::MappedMemory,
    memory::{SMemoryBlock, SMemoryHeapQueueElement, SMemoryImageBlock},
    memory_block::SDeviceMemoryBlock,
    phy_device::PhyDevice,
    queue::Queue,
    streamed_uniform::StreamedUniform,
    utils::{
        build_mipmaps, complete_buffer_object, complete_texture, copy_buffer, copy_buffer_to_image,
        get_memory_range,
    },
    vulkan_allocator::{
        FlushType, VulkanAllocator, VulkanDeviceInternalMemory, IMAGE_BUFFER_CACHE_ID,
        STAGING_BUFFER_CACHE_ID, STAGING_BUFFER_IMAGE_CACHE_ID,
    },
    vulkan_error::Error,
    vulkan_limits::Limits,
    vulkan_mem::{BufferAllocationError, ImageAllocationError, Memory},
    vulkan_types::{
        CTexture, EMemoryBlockUsage, ESupportedSamplerTypes, SBufferObject, SBufferObjectFrame,
        SDeviceDescriptorPools, SFrameBuffers, SStreamMemory, StreamMemory,
    },
    Options,
};

pub struct DeviceAsh {
    pub device: Arc<LogicalDevice>,
}

impl std::fmt::Debug for DeviceAsh {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DeviceAsh").finish()
    }
}

#[derive(Debug)]
pub struct Device {
    pub mem: Memory,
    pub mem_allocator: Arc<spin::Mutex<VulkanAllocator>>,
    pub streamed_uniform: Arc<spin::Mutex<StreamedUniform>>,

    pub ash_vk: DeviceAsh,

    pub vk_gpu: Arc<PhyDevice>,

    pub texture_memory_usage: Arc<AtomicU64>,
    pub buffer_memory_usage: Arc<AtomicU64>,
    pub stream_memory_usage: Arc<AtomicU64>,
    pub staging_memory_usage: Arc<AtomicU64>,

    pub non_flushed_staging_buffer_ranges: Vec<vk::MappedMemoryRange>,

    pub swap_chain_image_count: u32,

    pub samplers: [vk::Sampler; ESupportedSamplerTypes::Count as usize],
    pub textures: HashMap<u128, CTexture>,

    pub streamed_vertex_buffer: SStreamMemory<SFrameBuffers>,

    pub buffer_objects: HashMap<u128, SBufferObjectFrame>,

    pub standard_texture_descr_pool: SDeviceDescriptorPools,

    pub standard_textured_descriptor_set_layout: Arc<DescriptorSetLayout>,
    pub standard_3d_textured_descriptor_set_layout: Arc<DescriptorSetLayout>,

    pub sprite_multi_uniform_descriptor_set_layout: Arc<DescriptorSetLayout>,
    pub quad_uniform_descriptor_set_layout: Arc<DescriptorSetLayout>,

    // command buffers
    pub memory_command_buffers: Option<Rc<CommandBuffers>>,
    pub used_memory_command_buffer: Vec<bool>,

    pub global_texture_lod_bias: f64,

    pub is_headless: bool,

    _logger: SystemLogGroup,
}

impl Device {
    fn create_sprite_multi_uniform_descriptor_set_layout(
        device: &Arc<LogicalDevice>,
    ) -> anyhow::Result<Arc<DescriptorSetLayout>> {
        Ok(VulkanAllocator::create_uniform_descriptor_set_layout(
            device,
            vk::ShaderStageFlags::VERTEX,
        )?)
    }

    fn create_quad_uniform_descriptor_set_layout(
        device: &Arc<LogicalDevice>,
    ) -> anyhow::Result<Arc<DescriptorSetLayout>> {
        Ok(VulkanAllocator::create_uniform_descriptor_set_layout(
            device,
            vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
        )?)
    }

    fn create_descriptor_set_layouts(
        device: &Arc<LogicalDevice>,
    ) -> anyhow::Result<(Arc<DescriptorSetLayout>, Arc<DescriptorSetLayout>)> {
        let mut sampler_layout_binding = vk::DescriptorSetLayoutBinding::default();
        sampler_layout_binding.binding = 0;
        sampler_layout_binding.descriptor_count = 1;
        sampler_layout_binding.descriptor_type = vk::DescriptorType::COMBINED_IMAGE_SAMPLER;
        sampler_layout_binding.p_immutable_samplers = std::ptr::null();
        sampler_layout_binding.stage_flags = vk::ShaderStageFlags::FRAGMENT;

        let layout_bindings = [sampler_layout_binding];

        let mut layout_info = vk::DescriptorSetLayoutCreateInfo::default();
        layout_info.binding_count = layout_bindings.len() as u32;
        layout_info.p_bindings = layout_bindings.as_ptr();

        let standard_textured_descriptor_set_layout =
            DescriptorSetLayout::new(device.clone(), layout_info)?;

        let standard_3d_textured_descriptor_set_layout =
            DescriptorSetLayout::new(device.clone(), layout_info)?;
        Ok((
            standard_textured_descriptor_set_layout,
            standard_3d_textured_descriptor_set_layout,
        ))
    }

    pub fn new(
        dbg: Arc<AtomicU8>, // @see EDebugGFXModes
        instance: Arc<Instance>,
        device: Arc<LogicalDevice>,
        error: Arc<std::sync::Mutex<Error>>,
        vk_gpu: Arc<PhyDevice>,

        graphics_queue: Arc<spin::Mutex<Queue>>,

        texture_memory_usage: Arc<AtomicU64>,
        buffer_memory_usage: Arc<AtomicU64>,
        stream_memory_usage: Arc<AtomicU64>,
        staging_memory_usage: Arc<AtomicU64>,

        log: &SystemLog,
        is_headless: bool,
        options: &Options,

        thread_count: usize,
    ) -> anyhow::Result<Self> {
        let global_texture_lod_bias = options.gl.global_texture_lod_bias;

        let sprite_multi_uniform_descriptor_set_layout =
            Self::create_sprite_multi_uniform_descriptor_set_layout(&device)?;
        let quad_uniform_descriptor_set_layout =
            Self::create_quad_uniform_descriptor_set_layout(&device)?;

        let (standard_textured_descriptor_set_layout, standard_3d_textured_descriptor_set_layout) =
            Self::create_descriptor_set_layouts(&device)?;

        let mem = Memory::new(
            dbg.clone(),
            error.clone(),
            instance.clone(),
            device.clone(),
            vk_gpu.clone(),
            texture_memory_usage.clone(),
            buffer_memory_usage.clone(),
            stream_memory_usage.clone(),
            staging_memory_usage.clone(),
        );

        Ok(Self {
            mem: mem.clone(),
            mem_allocator: Arc::new(spin::Mutex::new(VulkanAllocator::new(
                device.clone(),
                mem.clone(),
                vk_gpu.limits.clone(),
                graphics_queue,
            )?)),
            streamed_uniform: StreamedUniform::new(
                device.clone(),
                mem.clone(),
                thread_count,
                sprite_multi_uniform_descriptor_set_layout.clone(),
                quad_uniform_descriptor_set_layout.clone(),
            )?,

            ash_vk: DeviceAsh {
                device: device.clone(),
            },

            vk_gpu,
            texture_memory_usage,
            buffer_memory_usage,
            stream_memory_usage,
            staging_memory_usage,
            non_flushed_staging_buffer_ranges: Default::default(),
            swap_chain_image_count: Default::default(),
            samplers: Default::default(),
            textures: Default::default(),
            streamed_vertex_buffer: Default::default(),
            buffer_objects: Default::default(),
            standard_texture_descr_pool: Default::default(),

            standard_textured_descriptor_set_layout,
            standard_3d_textured_descriptor_set_layout,

            sprite_multi_uniform_descriptor_set_layout,
            quad_uniform_descriptor_set_layout,

            memory_command_buffers: Default::default(),
            used_memory_command_buffer: Default::default(),
            global_texture_lod_bias,

            is_headless,

            _logger: log.logger("vulkan_device"),
        })
    }

    /************************
     * MEMORY MANAGEMENT
     ************************/
    pub fn upload_streamed_buffer<const FLUSH_FOR_RENDERING: bool, TName>(
        device: &ash::Device,
        non_coherent_mem_alignment: u64,
        streamed_buffer: &mut SStreamMemory<TName>,
        cur_image_index: u32,
    ) where
        TName: Clone + StreamMemory + std::fmt::Debug,
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
                    device
                        .flush_mapped_memory_ranges(
                            streamed_buffer
                                .get_ranges(cur_image_index as usize)
                                .split_at(range_update_count)
                                .0,
                        )
                        .unwrap();
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
        let upload_range = get_memory_range(buffer_mem, heap_data, &self.vk_gpu.limits);

        self.non_flushed_staging_buffer_ranges.push(upload_range);
    }

    pub fn prepare_staging_mem_range<const ID: usize>(&mut self, block: &SMemoryBlock<ID>) {
        self.prepare_staging_mem_range_impl(&block.buffer_mem, &block.heap_data);
    }

    pub fn upload_and_free_staging_mem_block(
        &mut self,
        block: SMemoryBlock<STAGING_BUFFER_CACHE_ID>,
    ) {
        self.prepare_staging_mem_range(&block);
    }

    pub fn upload_and_free_staging_image_mem_block(
        &mut self,
        block: SMemoryBlock<STAGING_BUFFER_IMAGE_CACHE_ID>,
    ) {
        self.prepare_staging_mem_range(&block);
    }

    pub fn get_memory_command_buffer(
        &mut self,
        res_mem_command_buffer: &mut *const vk::CommandBuffer,
        cur_image_index: u32,
    ) -> anyhow::Result<()> {
        let memory_command_buffer = &self
            .memory_command_buffers
            .as_ref()
            .ok_or(anyhow!("failed to get memory command buffer"))?
            .command_buffers[cur_image_index as usize];
        if !self.used_memory_command_buffer[cur_image_index as usize] {
            self.used_memory_command_buffer[cur_image_index as usize] = true;

            unsafe {
                self.ash_vk.device.device.reset_command_buffer(
                    *memory_command_buffer,
                    vk::CommandBufferResetFlags::RELEASE_RESOURCES,
                )
            }?;

            let mut begin_info = vk::CommandBufferBeginInfo::default();
            begin_info.flags = vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT;
            unsafe {
                self.ash_vk
                    .device
                    .device
                    .begin_command_buffer(*memory_command_buffer, &begin_info)?
            }
        }
        *res_mem_command_buffer = memory_command_buffer;
        Ok(())
    }

    pub fn memory_barrier(
        &mut self,
        buffer: &Arc<Buffer>,
        offset: vk::DeviceSize,
        size: vk::DeviceSize,
        buffer_access_type: vk::AccessFlags,
        before_command: bool,
        cur_image_index: u32,
    ) -> anyhow::Result<()> {
        let mut ptr_mem_command_buffer: *const vk::CommandBuffer = std::ptr::null_mut();
        self.get_memory_command_buffer(&mut ptr_mem_command_buffer, cur_image_index)?;
        let mem_command_buffer = unsafe { &*ptr_mem_command_buffer };

        memory_barrier(
            &self.ash_vk.device,
            *mem_command_buffer,
            buffer,
            offset,
            size,
            buffer_access_type,
            before_command,
        )
    }

    /************************
     * TEXTURES
     ************************/

    pub fn build_mipmaps(
        &mut self,
        image: &Arc<Image>,
        image_format: vk::Format,
        width: usize,
        height: usize,
        depth: usize,
        mip_map_level_count: usize,
        cur_image_index: u32,
    ) -> anyhow::Result<()> {
        let mut ptr_mem_command_buffer: *const vk::CommandBuffer = std::ptr::null_mut();
        self.get_memory_command_buffer(&mut ptr_mem_command_buffer, cur_image_index)?;
        let mem_command_buffer = unsafe { &*ptr_mem_command_buffer };

        build_mipmaps(
            &self.ash_vk.device,
            *mem_command_buffer,
            image,
            image_format,
            width,
            height,
            depth,
            mip_map_level_count,
        )
    }

    pub fn create_texture_image(
        &mut self,
        _image_index: u128,
        upload_data: VulkanDeviceInternalMemory,
        format: vk::Format,
        width: usize,
        height: usize,
        depth: usize,
        pixel_size: usize,
        mip_map_level_count: usize,
        cur_image_index: u32,
    ) -> anyhow::Result<(Arc<Image>, SMemoryImageBlock<IMAGE_BUFFER_CACHE_ID>), ImageAllocationError>
    {
        let allocator_dummy = self.mem_allocator.clone();
        let mut allocator = allocator_dummy.lock();
        let buffer_block_res =
            allocator.get_and_remove_mem_block_image(upload_data.mem.as_mut_ptr());

        let staging_and_image_buffer = buffer_block_res.unwrap();

        let (new_image, image_mem) = (
            staging_and_image_buffer.img,
            staging_and_image_buffer.img_mem,
        );

        let staging_buffer = staging_and_image_buffer.staging;
        // if not yet flushed. flush it
        if let FlushType::None = staging_and_image_buffer.is_flushed {
            self.prepare_staging_mem_range_impl(
                &staging_buffer.buffer_mem,
                &staging_buffer.heap_data,
            );
        }
        if let FlushType::StagingBufferFlushed | FlushType::None =
            staging_and_image_buffer.is_flushed
        {
            let mut ptr_mem_command_buffer: *const vk::CommandBuffer = std::ptr::null_mut();
            self.get_memory_command_buffer(&mut ptr_mem_command_buffer, cur_image_index)
                .map_err(|_| ImageAllocationError::MemoryRelatedOperationFailed)?;
            let mem_command_buffer = unsafe { &*ptr_mem_command_buffer };

            complete_texture(
                &self.ash_vk.device,
                *mem_command_buffer,
                &staging_buffer,
                &new_image,
                format,
                width,
                height,
                depth,
                pixel_size,
                mip_map_level_count,
            )?;
        }

        Ok((new_image, image_mem))
    }

    pub fn create_texture_image_view(
        &mut self,
        tex_image: &Arc<Image>,
        img_format: vk::Format,
        view_type: vk::ImageViewType,
        depth: usize,
        mip_map_level_count: usize,
    ) -> anyhow::Result<Arc<ImageView>> {
        self.create_image_view(
            tex_image,
            img_format,
            view_type,
            depth,
            mip_map_level_count,
            vk::ImageAspectFlags::COLOR,
        )
    }

    pub fn create_texture_samplers_impl(
        device: &ash::Device,
        max_sampler_anisotropy: u32,
        global_texture_lod_bias: f64,
        addr_mode_u: vk::SamplerAddressMode,
        addr_mode_v: vk::SamplerAddressMode,
        addr_mode_w: vk::SamplerAddressMode,
    ) -> anyhow::Result<vk::Sampler> {
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
        sampler_info.mip_lod_bias = global_texture_lod_bias as f32;
        sampler_info.min_lod = -1000.0;
        sampler_info.max_lod = 1000.0;

        Ok(unsafe { device.create_sampler(&sampler_info, None) }?)
    }

    pub fn create_image_view_swap_chain(
        &mut self,
        image: &dyn GetImg,
        format: vk::Format,
        view_type: vk::ImageViewType,
        depth: usize,
        mip_map_level_count: usize,
        aspect_mask: vk::ImageAspectFlags,
    ) -> anyhow::Result<vk::ImageView> {
        let mut view_create_info = vk::ImageViewCreateInfo::default();
        view_create_info.image = image.img();
        view_create_info.view_type = view_type;
        view_create_info.format = format;
        view_create_info.subresource_range.aspect_mask = aspect_mask;
        view_create_info.subresource_range.base_mip_level = 0;
        view_create_info.subresource_range.level_count = mip_map_level_count as u32;
        view_create_info.subresource_range.base_array_layer = 0;
        view_create_info.subresource_range.layer_count = depth as u32;

        Ok(unsafe {
            self.ash_vk
                .device
                .device
                .create_image_view(&view_create_info, None)
        }?)
    }

    pub fn create_image_view(
        &mut self,
        image: &Arc<Image>,
        format: vk::Format,
        view_type: vk::ImageViewType,
        depth: usize,
        mip_map_level_count: usize,
        aspect_mask: vk::ImageAspectFlags,
    ) -> anyhow::Result<Arc<ImageView>> {
        let mut view_create_info = vk::ImageViewCreateInfo::default();
        view_create_info.image = image.img();
        view_create_info.view_type = view_type;
        view_create_info.format = format;
        view_create_info.subresource_range.aspect_mask = aspect_mask;
        view_create_info.subresource_range.base_mip_level = 0;
        view_create_info.subresource_range.level_count = mip_map_level_count as u32;
        view_create_info.subresource_range.base_array_layer = 0;
        view_create_info.subresource_range.layer_count = depth as u32;

        Ok(ImageView::new(
            self.ash_vk.device.clone(),
            image.clone(),
            view_create_info,
        )?)
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

        vk::SampleCountFlags::TYPE_1
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

        vk::SampleCountFlags::TYPE_1
    }

    pub fn create_image(
        &mut self,
        width: u32,
        height: u32,
        depth: u32,
        mip_map_level_count: usize,
        format: vk::Format,
        tiling: vk::ImageTiling,
    ) -> anyhow::Result<(Arc<Image>, SMemoryImageBlock<IMAGE_BUFFER_CACHE_ID>), ImageAllocationError>
    {
        self.mem_allocator.lock().create_image_ex(
            width,
            height,
            depth,
            mip_map_level_count,
            format,
            tiling,
            vk::ImageUsageFlags::TRANSFER_SRC
                | vk::ImageUsageFlags::TRANSFER_DST
                | vk::ImageUsageFlags::SAMPLED,
            None,
            vk::ImageLayout::UNDEFINED,
        )
    }

    pub fn image_barrier(
        &mut self,
        image: &dyn GetImg,
        mip_map_base: usize,
        mip_map_count: usize,
        layer_base: usize,
        layer_count: usize,
        old_layout: vk::ImageLayout,
        new_layout: vk::ImageLayout,
        cur_image_index: u32,
    ) -> anyhow::Result<()> {
        let mut ptr_mem_command_buffer: *const vk::CommandBuffer = std::ptr::null_mut();
        self.get_memory_command_buffer(&mut ptr_mem_command_buffer, cur_image_index)
            .map_err(|err| {
                anyhow!("image barrier failed while getting memory command buffer: {err}")
            })?;
        let mem_command_buffer = unsafe { &*ptr_mem_command_buffer };

        image_barrier(
            &self.ash_vk.device,
            *mem_command_buffer,
            image,
            mip_map_base,
            mip_map_count,
            layer_base,
            layer_count,
            old_layout,
            new_layout,
        )
    }

    pub fn copy_buffer_to_image(
        &mut self,
        buffer: &Arc<Buffer>,
        buffer_offset: vk::DeviceSize,
        image: &Arc<Image>,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
        depth: usize,
        cur_image_index: u32,
    ) -> anyhow::Result<()> {
        let mut command_buffer_ptr: *const vk::CommandBuffer = std::ptr::null_mut();
        self.get_memory_command_buffer(&mut command_buffer_ptr, cur_image_index)
            .map_err(|err| {
                anyhow!("copy buffer to image failed during getting memory command buffer: {err}")
            })?;
        let command_buffer = unsafe { &*command_buffer_ptr };

        copy_buffer_to_image(
            &self.ash_vk.device,
            *command_buffer,
            buffer,
            buffer_offset,
            image,
            x,
            y,
            width,
            height,
            depth,
        )
    }

    /************************
     * STREAM BUFFERS SETUP
     ************************/
    pub fn create_index_buffer(
        &mut self,
        ptr_raw_data: *const c_void,
        data_size: usize,
        cur_image_index: u32,
    ) -> anyhow::Result<(Arc<Buffer>, Arc<SDeviceMemoryBlock>), BufferAllocationError> {
        let buffer_data_size = data_size as vk::DeviceSize;

        let staging_buffer = self
            .mem_allocator
            .lock()
            .get_staging_buffer(ptr_raw_data, data_size as u64)?;

        let (vertex_buffer, vertex_buffer_memory) = self.mem.create_buffer(
            buffer_data_size,
            EMemoryBlockUsage::Buffer,
            vk::BufferUsageFlags::TRANSFER_DST | vk::BufferUsageFlags::INDEX_BUFFER,
            vk::MemoryPropertyFlags::DEVICE_LOCAL,
        )?;

        self.memory_barrier(
            &vertex_buffer,
            0,
            buffer_data_size,
            vk::AccessFlags::INDEX_READ,
            true,
            cur_image_index,
        )
        .map_err(|_| BufferAllocationError::MemoryRelatedOperationFailed)?;
        self.copy_buffer(
            staging_buffer.buffer.as_ref().unwrap(),
            &vertex_buffer,
            staging_buffer.heap_data.offset_to_align as u64,
            0,
            buffer_data_size,
            cur_image_index,
        )
        .map_err(|_| BufferAllocationError::MemoryRelatedOperationFailed)?;
        self.memory_barrier(
            &vertex_buffer,
            0,
            buffer_data_size,
            vk::AccessFlags::INDEX_READ,
            false,
            cur_image_index,
        )
        .map_err(|_| BufferAllocationError::MemoryRelatedOperationFailed)?;

        self.upload_and_free_staging_mem_block(staging_buffer);

        Ok((vertex_buffer, vertex_buffer_memory))
    }
    /************************
     * BUFFERS
     ************************/
    pub fn create_stream_vertex_buffer(
        &mut self,
        data_size: usize,
        cur_image_index: u32,
    ) -> anyhow::Result<
        (
            *mut SFrameBuffers,
            Arc<Buffer>,
            Arc<SDeviceMemoryBlock>,
            usize,
            (isize, Arc<MappedMemory>),
        ),
        BufferAllocationError,
    > {
        VulkanAllocator::create_stream_buffer_unallocated::<
            SFrameBuffers,
            GlVertexTex3DStream,
            { StreamDataMax::MaxVertices as usize * 2 },
            1,
            false,
        >(
            &self.mem,
            &self.ash_vk.device,
            &mut |_: &mut SFrameBuffers, _: &Arc<Buffer>, _: vk::DeviceSize| -> bool { true },
            &mut self.streamed_vertex_buffer,
            vk::BufferUsageFlags::VERTEX_BUFFER,
            data_size,
            cur_image_index,
        )
    }

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

    pub fn create_buffer_object(
        &mut self,
        buffer_index: u128,
        upload_data: VulkanDeviceInternalMemory,
        buffer_data_size: vk::DeviceSize,
        cur_image_index: u32,
    ) -> anyhow::Result<(), BufferAllocationError> {
        let tmp_allocator = self.mem_allocator.clone();
        let mut allocator = tmp_allocator.lock();

        let staging_and_device_buffer = allocator
            .get_and_remove_mem_block(upload_data.mem.as_mut_ptr())
            .unwrap();

        let mem = staging_and_device_buffer.device;

        let staging_buffer = staging_and_device_buffer.staging;
        // if not yet flushed, flush it
        if let FlushType::None = staging_and_device_buffer.is_flushed {
            self.prepare_staging_mem_range_impl(
                &staging_buffer.buffer_mem,
                &staging_buffer.heap_data,
            );
        }
        if let FlushType::StagingBufferFlushed | FlushType::None =
            staging_and_device_buffer.is_flushed
        {
            let mut command_buffer_ptr: *const vk::CommandBuffer = std::ptr::null_mut();
            self.get_memory_command_buffer(&mut command_buffer_ptr, cur_image_index)
                .map_err(|_| BufferAllocationError::MemoryRelatedOperationFailed)?;
            let command_buffer = unsafe { &*command_buffer_ptr };

            complete_buffer_object(
                &self.ash_vk.device,
                *command_buffer,
                &staging_buffer,
                &mem,
                buffer_data_size,
            )?;
        }

        let vertex_buffer = mem.buffer.clone().unwrap();
        let buffer_offset = mem.heap_data.offset_to_align;
        self.buffer_objects.insert(
            buffer_index,
            SBufferObjectFrame {
                buffer_object: SBufferObject { mem },
                cur_buffer: vertex_buffer,
                cur_buffer_offset: buffer_offset,
            },
        );

        Ok(())
    }

    pub fn delete_buffer_object(&mut self, buffer_index: u128) {
        self.buffer_objects.remove(&buffer_index).unwrap();
    }

    pub fn copy_buffer(
        &mut self,
        src_buffer: &Arc<Buffer>,
        dst_buffer: &Arc<Buffer>,
        src_offset: vk::DeviceSize,
        dst_offset: vk::DeviceSize,
        copy_size: vk::DeviceSize,
        cur_image_index: u32,
    ) -> anyhow::Result<()> {
        let mut command_buffer_ptr: *const vk::CommandBuffer = std::ptr::null_mut();
        self.get_memory_command_buffer(&mut command_buffer_ptr, cur_image_index)
            .map_err(|err| {
                anyhow!("copy buffer failed while getting memory command buffer: {err}")
            })?;
        let command_buffer = unsafe { &*command_buffer_ptr };

        copy_buffer(
            &self.ash_vk.device,
            *command_buffer,
            src_buffer,
            dst_buffer,
            src_offset,
            dst_offset,
            copy_size,
        )
    }

    pub fn create_new_textured_descriptor_sets_impl(
        &mut self,
        img_view: vk::ImageView,
        sampler: vk::Sampler,
    ) -> anyhow::Result<Arc<DescriptorSet>, ImageAllocationError> {
        let descr_pool = &mut self.standard_texture_descr_pool;

        let mut des_alloc_info = vk::DescriptorSetAllocateInfo::default();
        des_alloc_info.descriptor_set_count = 1;
        des_alloc_info.p_set_layouts = &self.standard_textured_descriptor_set_layout.layout;

        let res = VulkanAllocator::get_descriptor_pool_for_alloc(
            &self.ash_vk.device,
            descr_pool,
            des_alloc_info,
            1,
        );
        if res.is_err() {
            return Err(ImageAllocationError::MemoryRelatedOperationFailed);
        }
        let descr_set = res.unwrap().remove(0);

        let mut image_info = vk::DescriptorImageInfo::default();
        image_info.image_layout = vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL;
        image_info.image_view = img_view;
        image_info.sampler = sampler;

        let mut descriptor_writes: [vk::WriteDescriptorSet; 1] = Default::default();
        descriptor_writes[0].dst_set = descr_set.set();
        descriptor_writes[0].dst_binding = 0;
        descriptor_writes[0].dst_array_element = 0;
        descriptor_writes[0].descriptor_type = vk::DescriptorType::COMBINED_IMAGE_SAMPLER;
        descriptor_writes[0].descriptor_count = 1;
        descriptor_writes[0].p_image_info = &image_info;

        unsafe {
            self.ash_vk
                .device
                .device
                .update_descriptor_sets(descriptor_writes.as_slice(), &[]);
        }

        Ok(descr_set)
    }

    pub fn create_new_textured_standard_descriptor_sets(
        &mut self,
        img_view: vk::ImageView,
        samplers: &[vk::Sampler; WRAP_TYPE_COUNT],
    ) -> anyhow::Result<[Arc<DescriptorSet>; WRAP_TYPE_COUNT], ImageAllocationError> {
        let set0 = self.create_new_textured_descriptor_sets_impl(img_view, samplers[0])?;
        let set1 = self.create_new_textured_descriptor_sets_impl(img_view, samplers[1])?;
        Ok([set0, set1])
    }

    pub fn create_new_3d_textured_standard_descriptor_sets(
        &mut self,
        img_3d_view: vk::ImageView,
        sampler_3d: vk::Sampler,
    ) -> anyhow::Result<Arc<DescriptorSet>, ImageAllocationError> {
        let mut des_alloc_info = vk::DescriptorSetAllocateInfo::default();
        des_alloc_info.descriptor_set_count = 1;
        des_alloc_info.p_set_layouts = &self.standard_3d_textured_descriptor_set_layout.layout;
        let res = VulkanAllocator::get_descriptor_pool_for_alloc(
            &self.ash_vk.device,
            &mut self.standard_texture_descr_pool,
            des_alloc_info,
            1,
        );
        if res.is_err() {
            return Err(ImageAllocationError::MemoryRelatedOperationFailed);
        }

        let descr_set = res.unwrap().remove(0);

        let mut image_info = vk::DescriptorImageInfo::default();
        image_info.image_layout = vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL;
        image_info.image_view = img_3d_view;
        image_info.sampler = sampler_3d;

        let mut descriptor_writes: [vk::WriteDescriptorSet; 1] = Default::default();

        descriptor_writes[0].dst_set = descr_set.set();
        descriptor_writes[0].dst_binding = 0;
        descriptor_writes[0].dst_array_element = 0;
        descriptor_writes[0].descriptor_type = vk::DescriptorType::COMBINED_IMAGE_SAMPLER;
        descriptor_writes[0].descriptor_count = 1;
        descriptor_writes[0].p_image_info = &image_info;

        unsafe {
            self.ash_vk
                .device
                .device
                .update_descriptor_sets(descriptor_writes.as_slice(), &[]);
        }

        Ok(descr_set)
    }

    pub fn get_texture_sampler(&self, sampler_type: ESupportedSamplerTypes) -> vk::Sampler {
        self.samplers[sampler_type as usize]
    }
}
