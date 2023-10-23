use std::{
    collections::{BTreeMap, HashMap},
    rc::Rc,
    sync::Arc,
};

use anyhow::anyhow;
use ash::vk;
use graphics_types::{
    command_buffer::TexFlags,
    types::{GraphicsBackendMemory, GraphicsMemoryAllocationType},
};
use libc::c_void;

use crate::backends::vulkan::utils::{complete_buffer_object, get_memory_range};

use super::{
    buffer::Buffer,
    command_buffer::CommandBuffers,
    command_pool::CommandPool,
    common::image_mip_level_count,
    descriptor_layout::DescriptorSetLayout,
    descriptor_pool::DescriptorPool,
    descriptor_set::DescriptorSet,
    fence::Fence,
    image::Image,
    logical_device::LogicalDevice,
    mapped_memory::MappedMemory,
    memory::{SMemoryBlock, SMemoryBlockCache, SMemoryImageBlock},
    memory_block::SDeviceMemoryBlock,
    queue::Queue,
    utils::complete_texture,
    vulkan_device::Device,
    vulkan_limits::Limits,
    vulkan_mem::{AllocationError, BufferAllocationError, ImageAllocationError, Memory},
    vulkan_types::{EMemoryBlockUsage, SDeviceDescriptorPools, SStreamMemory, StreamMemory},
};

// these caches are designed to be used outside of the backend
pub const STAGING_BUFFER_CACHE_ID: usize = 0;
pub const STAGING_BUFFER_IMAGE_CACHE_ID: usize = 1;
pub const VERTEX_BUFFER_CACHE_ID: usize = 2;
pub const IMAGE_BUFFER_CACHE_ID: usize = 3;

// good approximation of 1024x1024 image with mipmaps
pub const IMG_SIZE1024X1024: i64 = (1024 * 1024 * 4) * 2;

#[derive(Debug)]
pub enum FlushType {
    None,
    StagingBufferFlushed,
    FullyCreated,
}

#[derive(Debug)]
pub struct VulkanAllocatorBufferCacheEntry {
    pub staging: SMemoryBlock<STAGING_BUFFER_CACHE_ID>,
    pub device: SMemoryBlock<VERTEX_BUFFER_CACHE_ID>,

    pub is_flushed: FlushType,
}

#[derive(Debug, Clone, Copy)]
pub struct VulkanAllocatorImageCacheEntryData {
    pub width: usize,
    pub height: usize,
    pub depth: usize,
    pub is_3d_tex: bool,
    pub flags: TexFlags,
    pub mip_map_count: usize,
}

#[derive(Debug)]
pub struct VulkanAllocatorImageCacheEntry {
    pub staging: SMemoryBlock<STAGING_BUFFER_IMAGE_CACHE_ID>,
    pub img: Arc<Image>,
    pub img_mem: SMemoryImageBlock<IMAGE_BUFFER_CACHE_ID>,

    pub data: VulkanAllocatorImageCacheEntryData,

    pub is_flushed: FlushType,
}

pub struct VulkanDeviceInternalMemory {
    pub(crate) mem: &'static mut [u8],
}

#[derive(Debug, Default)]
pub struct VulkanAllocatorPointerWork {
    mapped_memory_cache: HashMap<std::ptr::NonNull<u8>, VulkanAllocatorBufferCacheEntry>,
    mapped_memory_cache_image: HashMap<std::ptr::NonNull<u8>, VulkanAllocatorImageCacheEntry>,
}

unsafe impl Send for VulkanAllocatorPointerWork {}
unsafe impl Sync for VulkanAllocatorPointerWork {}

#[derive(Debug)]
pub struct VulkanAllocatorLocalData {
    command_buffers: Rc<CommandBuffers>,
}

unsafe impl Send for VulkanAllocatorLocalData {}
unsafe impl Sync for VulkanAllocatorLocalData {}

/**
 * The vulkan allocator struct is specifically designed to be
 * used in a multi threaded scenario outside of the backend
 */
#[derive(Debug)]
pub struct VulkanAllocator {
    pub mem: Memory,
    pub staging_buffer_cache: SMemoryBlockCache<{ STAGING_BUFFER_CACHE_ID }>,
    pub staging_buffer_cache_image: SMemoryBlockCache<{ STAGING_BUFFER_IMAGE_CACHE_ID }>,
    pub vertex_buffer_cache: SMemoryBlockCache<{ VERTEX_BUFFER_CACHE_ID }>,
    pub image_buffer_caches: BTreeMap<u32, SMemoryBlockCache<{ IMAGE_BUFFER_CACHE_ID }>>,

    pub limits: Limits,

    pub frame_count: u32,

    // private
    device: Arc<LogicalDevice>,
    ptr_work: VulkanAllocatorPointerWork,
    queue: Arc<spin::Mutex<Queue>>,

    local: VulkanAllocatorLocalData,
    fence: Arc<Fence>,
}

impl VulkanAllocator {
    pub fn new(
        logical_device: Arc<LogicalDevice>,
        mem: Memory,
        limits: Limits,
        graphics_queue: Arc<spin::Mutex<Queue>>,
    ) -> anyhow::Result<Self> {
        let command_pool = CommandPool::new(
            logical_device.clone(),
            logical_device.phy_device.queue_node_index,
            1,
            0,
        )?;
        let command_buffers =
            CommandBuffers::new(command_pool.clone(), vk::CommandBufferLevel::PRIMARY, 1)?;
        let fence = Fence::new(logical_device.clone())?;
        Ok(Self {
            device: logical_device,
            mem,
            staging_buffer_cache: Default::default(),
            staging_buffer_cache_image: Default::default(),
            image_buffer_caches: Default::default(),
            vertex_buffer_cache: Default::default(),
            limits,

            frame_count: 0,

            ptr_work: Default::default(),
            queue: graphics_queue,

            local: VulkanAllocatorLocalData { command_buffers },
            fence,
        })
    }

    pub fn set_frame_count(&mut self, frame_count: usize) {
        self.frame_count = frame_count as u32
    }

    pub fn set_frame_index(&mut self, frame_index: usize) {
        let cur_frame_index = frame_index as u32;
        self.vertex_buffer_cache.set_frame_index(cur_frame_index);
        self.staging_buffer_cache.set_frame_index(cur_frame_index);
        self.staging_buffer_cache_image
            .set_frame_index(cur_frame_index);
        for img_cache in &mut self.image_buffer_caches {
            img_cache.1.set_frame_index(cur_frame_index);
        }
    }

    /************************
     * MEMORY MANAGEMENT
     ************************/
    pub fn memory_to_internal_memory(
        &mut self,
        mem: GraphicsBackendMemory,
        usage: GraphicsMemoryAllocationType,
    ) -> anyhow::Result<VulkanDeviceInternalMemory, (GraphicsBackendMemory, AllocationError)> {
        match mem {
            GraphicsBackendMemory::Static(mut mem) => {
                mem.deallocator = None;
                let mem = mem.mem.take().unwrap();
                let exists = match usage {
                    GraphicsMemoryAllocationType::Texture { .. } => {
                        self.mem_block_image_exists(mem.as_ptr() as *mut _)
                    }
                    GraphicsMemoryAllocationType::Buffer { .. } => {
                        self.mem_blocke_exists(mem.as_ptr() as *mut _)
                    }
                };

                if !exists {
                    panic!(
                        "memory block was not of correct type (requested type: {:?})",
                        usage
                    );
                }

                Ok(VulkanDeviceInternalMemory { mem })
            }
            GraphicsBackendMemory::Vector(m) => match usage {
                GraphicsMemoryAllocationType::Buffer { .. } => {
                    let res = self
                        .get_staging_buffer_for_mem_alloc(m.as_ptr() as *const _, m.len() as u64)
                        .map_err(|err| (GraphicsBackendMemory::Vector(m), err.into()))?;
                    Ok(VulkanDeviceInternalMemory { mem: res })
                }
                GraphicsMemoryAllocationType::Texture {
                    width,
                    height,
                    depth,
                    is_3d_tex,
                    flags,
                } => {
                    let res = self
                        .get_staging_buffer_image_for_mem_alloc(
                            m.as_ptr() as *const _,
                            width,
                            height,
                            depth,
                            is_3d_tex,
                            flags,
                        )
                        .map_err(|err| (GraphicsBackendMemory::Vector(m), err.into()))?;

                    Ok(VulkanDeviceInternalMemory { mem: res })
                }
            },
        }
    }

    pub fn shrink_unused_caches(&mut self) {
        self.staging_buffer_cache.shrink();
        self.staging_buffer_cache_image.shrink();

        self.vertex_buffer_cache.shrink();

        for image_buffer_cache in &mut self.image_buffer_caches {
            image_buffer_cache.1.shrink();
        }
    }

    pub fn clear_frame_data(&mut self, frame_image_index: usize) {
        self.staging_buffer_cache.cleanup(frame_image_index);
        self.staging_buffer_cache_image.cleanup(frame_image_index);
        self.vertex_buffer_cache.cleanup(frame_image_index);
        for image_buffer_cache in &mut self.image_buffer_caches {
            image_buffer_cache.1.cleanup(frame_image_index);
        }
    }

    pub fn destroy_frame_data(&mut self) {
        self.staging_buffer_cache
            .destroy_frame_data(self.frame_count as usize);
        self.staging_buffer_cache_image
            .destroy_frame_data(self.frame_count as usize);
        self.vertex_buffer_cache
            .destroy_frame_data(self.frame_count as usize);
        for image_buffer_cache in &mut self.image_buffer_caches {
            image_buffer_cache
                .1
                .destroy_frame_data(self.frame_count as usize);
        }
    }

    pub fn init_caches(&mut self) {
        self.staging_buffer_cache.init(self.frame_count as usize);
        self.staging_buffer_cache_image
            .init(self.frame_count as usize);
        self.vertex_buffer_cache.init(self.frame_count as usize);
    }

    pub fn destroy_caches(&mut self) {
        self.staging_buffer_cache.destroy();
        self.staging_buffer_cache_image.destroy();
        self.vertex_buffer_cache.destroy();
        for image_buffer_cache in &mut self.image_buffer_caches {
            image_buffer_cache.1.destroy();
        }

        self.image_buffer_caches.clear();
    }

    pub fn get_image_memory(
        &mut self,
        required_size: vk::DeviceSize,
        required_alignment: vk::DeviceSize,
        required_memory_type_bits: u32,
    ) -> anyhow::Result<SMemoryImageBlock<IMAGE_BUFFER_CACHE_ID>, BufferAllocationError> {
        let it = self.image_buffer_caches.get_mut(&required_memory_type_bits);
        let mem: &mut SMemoryBlockCache<{ IMAGE_BUFFER_CACHE_ID }>;
        match it {
            None => {
                self.image_buffer_caches
                    .insert(required_memory_type_bits, SMemoryBlockCache::default());

                mem = self
                    .image_buffer_caches
                    .get_mut(&required_memory_type_bits)
                    .unwrap();
                mem.init(self.frame_count as usize);
            }
            Some(it) => {
                mem = it;
            }
        }
        self.mem
            .get_image_memory_block_impl::<IMAGE_BUFFER_CACHE_ID, IMG_SIZE1024X1024, 2>(
                mem,
                vk::MemoryPropertyFlags::DEVICE_LOCAL,
                required_size,
                required_alignment,
                required_memory_type_bits,
            )
    }

    pub fn get_vertex_buffer(
        &mut self,
        required_size: vk::DeviceSize,
    ) -> anyhow::Result<SMemoryBlock<VERTEX_BUFFER_CACHE_ID>, BufferAllocationError> {
        self.mem
            .get_buffer_block_impl::<{ VERTEX_BUFFER_CACHE_ID }, { 8 * 1024 * 1024 }, 3, false>(
                &mut self.vertex_buffer_cache,
                vk::BufferUsageFlags::TRANSFER_DST | vk::BufferUsageFlags::VERTEX_BUFFER,
                vk::MemoryPropertyFlags::DEVICE_LOCAL,
                std::ptr::null(),
                required_size,
                16,
            )
    }

    pub fn create_image_ex(
        &mut self,
        width: u32,
        height: u32,
        depth: u32,
        mip_map_level_count: usize,
        format: vk::Format,
        tiling: vk::ImageTiling,
        image_usage: vk::ImageUsageFlags,
        sample_count: Option<u32>,
        initial_layout: vk::ImageLayout,
    ) -> anyhow::Result<(Arc<Image>, SMemoryImageBlock<IMAGE_BUFFER_CACHE_ID>), ImageAllocationError>
    {
        let mut image_info = vk::ImageCreateInfo::default();
        image_info.image_type = vk::ImageType::TYPE_2D;
        image_info.extent.width = width;
        image_info.extent.height = height;
        image_info.extent.depth = 1;
        image_info.mip_levels = mip_map_level_count as u32;
        image_info.array_layers = depth;
        image_info.format = format;
        image_info.tiling = tiling;
        image_info.initial_layout = initial_layout;
        image_info.usage = image_usage;
        image_info.samples = if let Some(sample_count) = sample_count {
            Device::get_sample_count(sample_count, &self.limits)
        } else {
            vk::SampleCountFlags::TYPE_1
        };
        image_info.sharing_mode = vk::SharingMode::EXCLUSIVE;

        let image = Image::new(self.device.clone(), image_info)?;

        let mem_requirements = unsafe {
            self.device
                .device
                .get_image_memory_requirements(image.image)
        };

        let image_memory = self.get_image_memory(
            mem_requirements.size,
            mem_requirements.alignment,
            mem_requirements.memory_type_bits,
        )?;

        image.bind(
            image_memory.base.buffer_mem.clone(),
            image_memory.base.heap_data.offset_to_align as u64,
        )?;

        Ok((image, image_memory))
    }

    pub fn get_staging_buffer(
        &mut self,
        buffer_data: *const c_void,
        required_size: vk::DeviceSize,
    ) -> anyhow::Result<SMemoryBlock<STAGING_BUFFER_CACHE_ID>, BufferAllocationError> {
        self.mem
            .get_buffer_block_impl::<{ STAGING_BUFFER_CACHE_ID }, { 8 * 1024 * 1024 }, 3, true>(
                &mut self.staging_buffer_cache,
                vk::BufferUsageFlags::TRANSFER_SRC,
                vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_CACHED,
                buffer_data,
                required_size,
                std::cmp::max::<vk::DeviceSize>(self.limits.non_coherent_mem_alignment, 16),
            )
    }

    /// returns buffer, buffer mem, offset & memory mapped ptr
    pub fn create_stream_buffer_unallocated<
        'a,
        TStreamMemName: Clone,
        TInstanceTypeName,
        const INSTANCE_TYPE_COUNT: usize,
        const BUFFER_CREATE_COUNT: usize,
        const USES_CURRENT_COUNT_OFFSET: bool,
    >(
        mem: &Memory,
        device: &Arc<LogicalDevice>,
        new_mem_func: &'a mut dyn FnMut(&mut TStreamMemName, &Arc<Buffer>, vk::DeviceSize) -> bool,
        stream_uniform_buffer: &mut SStreamMemory<TStreamMemName>,
        usage: vk::BufferUsageFlags,
        data_size: usize,
        cur_image_index: u32,
    ) -> anyhow::Result<
        (
            *mut TStreamMemName,
            Arc<Buffer>,
            Arc<SDeviceMemoryBlock>,
            usize,
            (isize, Arc<MappedMemory>),
        ),
        BufferAllocationError,
    >
    where
        TStreamMemName: StreamMemory + std::fmt::Debug,
    {
        let mut buffer = None;
        let mut buffer_mem = None;
        let mut offset = 0;
        let mut ptr_mem = None;
        let mut ptr_buffer_mem = None;

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
                buffer = Some(buffer_of_frame.get_buffer().clone());
                buffer_mem = Some(buffer_of_frame.get_device_mem_block().clone());
                offset = *buffer_of_frame.get_used_size();
                *buffer_of_frame.get_used_size() += data_size;
                let mapped_data = buffer_of_frame.get_mapped_buffer_data();
                ptr_mem = Some((mapped_data.0, mapped_data.1.clone()));
                ptr_buffer_mem = Some(it);
                break;
            }
            it += 1;
        }

        if buffer_mem.is_none() {
            // create memory
            let new_buffer_single_size =
                (std::mem::size_of::<TInstanceTypeName>() * INSTANCE_TYPE_COUNT) as vk::DeviceSize;
            let new_buffer_size =
                (new_buffer_single_size * BUFFER_CREATE_COUNT as u64) as vk::DeviceSize;
            let (stream_buffer, stream_buffer_memory) = mem.create_buffer(
                new_buffer_size,
                EMemoryBlockUsage::Stream,
                usage,
                vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_CACHED,
            )?;

            let ptr_mapped_data =
                MappedMemory::new(device.clone(), stream_buffer_memory.clone(), 0).unwrap();

            let new_buffer_index: usize = stream_uniform_buffer
                .get_buffers(cur_image_index as usize)
                .len();
            for i in 0..BUFFER_CREATE_COUNT {
                stream_uniform_buffer
                    .get_buffers(cur_image_index as usize)
                    .push(TStreamMemName::new(
                        stream_buffer.clone(),
                        stream_buffer_memory.clone(),
                        new_buffer_single_size as usize * i,
                        new_buffer_single_size as usize,
                        0,
                        (
                            (new_buffer_single_size as isize * i as isize) as isize,
                            ptr_mapped_data.clone(),
                        ),
                    ));
                stream_uniform_buffer
                    .get_ranges(cur_image_index as usize)
                    .push(Default::default());
                if !new_mem_func(
                    stream_uniform_buffer
                        .get_buffers(cur_image_index as usize)
                        .last_mut()
                        .unwrap(),
                    &stream_buffer,
                    new_buffer_single_size * i as u64,
                ) {
                    return Err(BufferAllocationError::MemoryRelatedOperationFailed);
                }
            }

            stream_uniform_buffer.increase_used_count(cur_image_index as usize);
            let new_stream_buffer =
                &mut stream_uniform_buffer.get_buffers(cur_image_index as usize)[new_buffer_index];

            let mapped_data = new_stream_buffer.get_mapped_buffer_data();
            ptr_mem = Some((mapped_data.0, mapped_data.1.clone()));
            offset = *new_stream_buffer.get_offset_in_buffer();
            *new_stream_buffer.get_used_size() += data_size;
            new_stream_buffer.set_is_used();

            ptr_buffer_mem = Some(new_buffer_index);
            buffer = Some(stream_buffer);
            buffer_mem = Some(stream_buffer_memory);
        }
        Ok((
            &mut stream_uniform_buffer.get_buffers(cur_image_index as usize)
                [ptr_buffer_mem.unwrap()],
            buffer.unwrap(),
            buffer_mem.unwrap(),
            offset,
            ptr_mem.unwrap(),
        ))
    }

    // returns true, if the stream memory was just allocated
    pub fn create_stream_buffer<
        'a,
        TStreamMemName: Clone,
        TInstanceTypeName,
        const INSTANCE_TYPE_COUNT: usize,
        const BUFFER_CREATE_COUNT: usize,
        const USES_CURRENT_COUNT_OFFSET: bool,
    >(
        mem: &Memory,
        device: &Arc<LogicalDevice>,
        new_mem_func: &'a mut dyn FnMut(&mut TStreamMemName, &Arc<Buffer>, vk::DeviceSize) -> bool,
        stream_uniform_buffer: &mut SStreamMemory<TStreamMemName>,
        usage: vk::BufferUsageFlags,
        ptr_raw_data: *const c_void,
        data_size: usize,
        cur_image_index: u32,
    ) -> anyhow::Result<
        (
            *mut TStreamMemName,
            Arc<Buffer>,
            Arc<SDeviceMemoryBlock>,
            usize,
        ),
        BufferAllocationError,
    >
    where
        TStreamMemName: StreamMemory + std::fmt::Debug,
    {
        let (ptr_buffer_mem, buffer, buffer_mem, offset, ptr_mem) =
            Self::create_stream_buffer_unallocated::<
                TStreamMemName,
                TInstanceTypeName,
                INSTANCE_TYPE_COUNT,
                BUFFER_CREATE_COUNT,
                USES_CURRENT_COUNT_OFFSET,
            >(
                mem,
                device,
                new_mem_func,
                stream_uniform_buffer,
                usage,
                data_size,
                cur_image_index,
            )?;

        unsafe {
            libc::memcpy(
                ptr_mem
                    .1
                    .get_mem()
                    .offset(ptr_mem.0)
                    .offset(offset as isize) as *mut c_void,
                ptr_raw_data,
                data_size,
            );
        }

        Ok((ptr_buffer_mem, buffer, buffer_mem, offset))
    }

    pub fn allocate_descriptor_pool(
        device: &Arc<LogicalDevice>,
        descriptor_pools: &mut SDeviceDescriptorPools,
        alloc_pool_size: usize,
    ) -> anyhow::Result<()> {
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

        let res = DescriptorPool::new(device.clone(), pool_info);
        if res.is_err() {
            return Err(anyhow!("Creating the descriptor pool failed."));
        }

        descriptor_pools.pools.push(res.unwrap());

        Ok(())
    }

    pub fn get_descriptor_pool_for_alloc(
        device: &Arc<LogicalDevice>,
        descriptor_pools: &mut SDeviceDescriptorPools,
        set_create_info_without_pool: vk::DescriptorSetAllocateInfo,
        alloc_num: usize,
    ) -> anyhow::Result<Vec<Arc<DescriptorSet>>> {
        let mut cur_alloc_num = alloc_num;

        let mut pool_index_offset = 0;
        let mut pool_size_of_current_index = 0;

        let mut res: Vec<Arc<DescriptorSet>> = Vec::new();

        while cur_alloc_num > 0 {
            let mut allocated_in_this_run = 0;

            let mut found = false;
            let mut descriptor_pool_index = usize::MAX;
            for i in pool_index_offset..descriptor_pools.pools.len() {
                let pool = &descriptor_pools.pools[i];
                if pool.cur_size.load(std::sync::atomic::Ordering::SeqCst)
                    + pool_size_of_current_index
                    + (cur_alloc_num as u64)
                    < pool.size
                {
                    allocated_in_this_run = cur_alloc_num;
                    found = true;
                    descriptor_pool_index = i;
                    break;
                } else {
                    let remaining_pool_count = pool.size
                        - (pool.cur_size.load(std::sync::atomic::Ordering::SeqCst)
                            + pool_size_of_current_index);
                    if remaining_pool_count > 0 {
                        allocated_in_this_run = remaining_pool_count as usize;
                        found = true;
                        descriptor_pool_index = i;
                        break;
                    }
                }
            }

            if !found {
                descriptor_pool_index = descriptor_pools.pools.len();

                Self::allocate_descriptor_pool(
                    device,
                    descriptor_pools,
                    descriptor_pools.default_alloc_size as usize,
                )?;

                allocated_in_this_run =
                    std::cmp::min(descriptor_pools.default_alloc_size as usize, cur_alloc_num);
            }

            for _ in 0..allocated_in_this_run {
                let pool = descriptor_pools.pools[descriptor_pool_index].clone();

                let new_descr = DescriptorSet::new(pool, set_create_info_without_pool)?;
                res.push(new_descr);
            }

            if descriptor_pool_index != pool_index_offset {
                pool_index_offset = descriptor_pool_index;
                pool_size_of_current_index = 0;
            }
            pool_size_of_current_index += allocated_in_this_run as u64;

            cur_alloc_num -= allocated_in_this_run;
        }

        Ok(res)
    }

    pub fn create_uniform_descriptor_sets(
        device: &Arc<LogicalDevice>,
        descr_pools: &mut SDeviceDescriptorPools,
        set_layout: &Arc<DescriptorSetLayout>,
        set_count: usize,
        bind_buffer: &Arc<Buffer>,
        single_buffer_instance_size: usize,
        memory_offset: vk::DeviceSize,
    ) -> anyhow::Result<Vec<Arc<DescriptorSet>>> {
        let mut des_alloc_info = vk::DescriptorSetAllocateInfo::default();
        des_alloc_info.descriptor_set_count = 1;
        des_alloc_info.p_set_layouts = &set_layout.layout;
        let descriptors =
            Self::get_descriptor_pool_for_alloc(device, descr_pools, des_alloc_info, set_count)?;
        for i in 0..descriptors.len() {
            let mut buffer_info = vk::DescriptorBufferInfo::default();
            buffer_info.buffer = bind_buffer.buffer;
            buffer_info.offset = memory_offset + (single_buffer_instance_size * i) as u64;
            buffer_info.range = single_buffer_instance_size as u64;

            let mut descriptor_writes: [vk::WriteDescriptorSet; 1] = Default::default();
            descriptor_writes[0].dst_set = descriptors[i].set();
            descriptor_writes[0].dst_binding = 1;
            descriptor_writes[0].dst_array_element = 0;
            descriptor_writes[0].descriptor_type = vk::DescriptorType::UNIFORM_BUFFER;
            descriptor_writes[0].descriptor_count = 1;
            descriptor_writes[0].p_buffer_info = &buffer_info;

            unsafe {
                device
                    .device
                    .update_descriptor_sets(&descriptor_writes, &[]);
            }
        }

        Ok(descriptors)
    }

    pub fn create_uniform_descriptor_set_layout(
        device: &Arc<LogicalDevice>,
        stage_flags: vk::ShaderStageFlags,
    ) -> anyhow::Result<Arc<DescriptorSetLayout>> {
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

        Ok(DescriptorSetLayout::new(device.clone(), layout_info)?)
    }

    pub fn get_staging_buffer_image(
        &mut self,
        mem: &Memory,
        limits: &Limits,
        buffer_data: &[u8],
        required_size: vk::DeviceSize,
    ) -> anyhow::Result<SMemoryBlock<STAGING_BUFFER_IMAGE_CACHE_ID>, BufferAllocationError> {
        mem
            .get_buffer_block_impl::<{ STAGING_BUFFER_IMAGE_CACHE_ID }, { 8 * 1024 * 1024 }, 3, true>(
                &mut self.staging_buffer_cache_image,
                vk::BufferUsageFlags::TRANSFER_SRC,
                vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_CACHED,
                buffer_data.as_ptr() as *const c_void,
                required_size,
                std::cmp::max::<vk::DeviceSize>(
                    limits.optimal_image_copy_mem_alignment,
                    std::cmp::max::<vk::DeviceSize>(limits.non_coherent_mem_alignment, 16),
                )
            )
    }

    pub fn get_staging_buffer_for_mem_alloc(
        &mut self,
        buffer_data: *const c_void,
        required_size: vk::DeviceSize,
    ) -> anyhow::Result<&'static mut [u8], BufferAllocationError> {
        let res_block = self
            .mem
            .get_buffer_block_impl::<{ STAGING_BUFFER_CACHE_ID }, { 8 * 1024 * 1024 }, 3, true>(
                &mut self.staging_buffer_cache,
                vk::BufferUsageFlags::TRANSFER_SRC,
                vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_CACHED,
                buffer_data,
                required_size,
                std::cmp::max::<vk::DeviceSize>(self.limits.non_coherent_mem_alignment, 16),
            )?;

        let res_buffer = self.get_vertex_buffer(required_size)?;

        let res = unsafe {
            let mem = res_block.mapped_buffer.as_ref().unwrap();
            std::slice::from_raw_parts_mut(mem.1.get_mem().offset(mem.0), required_size as usize)
        };

        self.ptr_work.mapped_memory_cache.insert(
            std::ptr::NonNull::new(res.as_mut_ptr()).unwrap(),
            VulkanAllocatorBufferCacheEntry {
                staging: res_block,
                device: res_buffer,

                is_flushed: FlushType::None,
            },
        );

        Ok(res)
    }

    pub fn get_staging_buffer_image_for_mem_alloc(
        &mut self,
        buffer_data: *const c_void,

        width: usize,
        height: usize,
        depth: usize,
        is_3d_tex: bool,
        flags: TexFlags,
    ) -> anyhow::Result<&'static mut [u8], ImageAllocationError> {
        if width as u32 > self.limits.max_texture_size
            || height as u32 > self.limits.max_texture_size
            || depth as u32 > self.limits.max_texture_size
            || (width * height * depth)
                > (self.limits.max_texture_size as usize * self.limits.max_texture_size as usize)
        {
            return Err(ImageAllocationError::ImageDimensionsTooBig);
        }

        let res_block = self.mem
             .get_buffer_block_impl::<{ STAGING_BUFFER_IMAGE_CACHE_ID }, { 8 * 1024 * 1024 }, 3, true>(
                                  &mut self.staging_buffer_cache_image,
                 vk::BufferUsageFlags::TRANSFER_SRC,
                 vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_CACHED,
                 buffer_data,
                 (width*height*depth*4) as vk::DeviceSize,
                 std::cmp::max::<vk::DeviceSize>(
                     self.limits.optimal_image_copy_mem_alignment,
                     std::cmp::max::<vk::DeviceSize>(self.limits.non_coherent_mem_alignment, 16),
                 )
             )?;

        let requires_mip_maps = (flags & TexFlags::TEXFLAG_NOMIPMAPS).is_empty();
        let mut mip_map_level_count: usize = 1;
        if requires_mip_maps {
            let img_size = vk::Extent3D {
                width: width as u32,
                height: height as u32,
                depth: 1,
            };
            mip_map_level_count = image_mip_level_count(img_size);
            if !self
                .device
                .phy_device
                .config
                .read()
                .unwrap()
                .optimal_rgba_image_blitting
            {
                mip_map_level_count = 1;
            }
        }

        let (new_image, image_mem) = self.create_image_ex(
            width as u32,
            height as u32,
            depth as u32,
            mip_map_level_count,
            vk::Format::R8G8B8A8_UNORM,
            vk::ImageTiling::OPTIMAL,
            vk::ImageUsageFlags::TRANSFER_SRC
                | vk::ImageUsageFlags::TRANSFER_DST
                | vk::ImageUsageFlags::SAMPLED,
            None,
            vk::ImageLayout::UNDEFINED,
        )?;

        let res = unsafe {
            let mem = res_block.mapped_buffer.as_ref().unwrap();
            std::slice::from_raw_parts_mut(
                mem.1.get_mem().offset(mem.0),
                width * height * depth * 4,
            )
        };

        self.ptr_work.mapped_memory_cache_image.insert(
            std::ptr::NonNull::new(res.as_mut_ptr()).unwrap(),
            VulkanAllocatorImageCacheEntry {
                staging: res_block,
                img: new_image,
                img_mem: image_mem,

                data: VulkanAllocatorImageCacheEntryData {
                    width,
                    height,
                    depth,
                    is_3d_tex,
                    flags,
                    mip_map_count: mip_map_level_count,
                },

                is_flushed: FlushType::None,
            },
        );

        Ok(res)
    }

    pub fn free_mem_raw(&mut self, mem: *mut u8) {
        // try to find the buffer in the buffer cache first
        let res = self
            .ptr_work
            .mapped_memory_cache
            .remove(&std::ptr::NonNull::new(mem).unwrap());
        if let None = res {
            let res = self
                .ptr_work
                .mapped_memory_cache_image
                .remove(&std::ptr::NonNull::new(mem).unwrap());
            if let None = res {
                panic!("memory that was tried to be deallocated was not found. That could mean it was already free'd (dobule free).");
            }
        }
    }

    fn start_command_buffer(
        device: &Arc<LogicalDevice>,
        command_buffers: &Rc<CommandBuffers>,
    ) -> anyhow::Result<()> {
        let mut begin_info = vk::CommandBufferBeginInfo::default();
        begin_info.flags = vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT;
        unsafe {
            device
                .device
                .begin_command_buffer(command_buffers.command_buffers[0], &begin_info)
        }?;

        Ok(())
    }

    fn execute_command_buffer(
        device: &Arc<LogicalDevice>,
        fence: &Fence,
        command_buffers: &Rc<CommandBuffers>,
        queue: &Arc<spin::Mutex<Queue>>,
    ) -> anyhow::Result<(vk::Fence, vk::CommandBuffer, ash::Device)> {
        unsafe {
            device
                .device
                .end_command_buffer(command_buffers.command_buffers[0])?;
        }

        let mut submit_info = vk::SubmitInfo::default();

        let command_buffers = [command_buffers.command_buffers[0]];
        submit_info.command_buffer_count = command_buffers.len() as u32;
        submit_info.p_command_buffers = command_buffers.as_ptr();
        unsafe {
            device.device.reset_fences(&[fence.fence])?;
            let queue = queue.lock();
            device
                .device
                .queue_submit(queue.graphics_queue, &[submit_info], fence.fence)?;
        }

        Ok((fence.fence, command_buffers[0], device.device.clone()))
    }

    pub fn flush_img_memory(
        &mut self,
        mem: *mut u8,
        full_flush: bool,
    ) -> anyhow::Result<Option<(vk::Fence, vk::CommandBuffer, ash::Device)>> {
        if let Some(img) = self
            .ptr_work
            .mapped_memory_cache_image
            .get_mut(&std::ptr::NonNull::new(mem).unwrap())
        {
            // flush the staging buffer
            let upload_range = get_memory_range(
                &img.staging.buffer_mem,
                &img.staging.heap_data,
                &self.limits,
            );
            unsafe {
                self.device
                    .device
                    .flush_mapped_memory_ranges(&[upload_range])
                    .unwrap();
            }

            let res = if full_flush {
                Self::start_command_buffer(&self.device, &self.local.command_buffers)?;

                complete_texture(
                    &self.device,
                    self.local.command_buffers.command_buffers[0],
                    &img.staging,
                    &img.img,
                    vk::Format::R8G8B8A8_UNORM,
                    img.data.width,
                    img.data.height,
                    img.data.depth,
                    4,
                    img.data.mip_map_count,
                )?;

                let res = Self::execute_command_buffer(
                    &self.device,
                    &self.fence,
                    &self.local.command_buffers,
                    &self.queue,
                )?;

                img.is_flushed = FlushType::FullyCreated;

                Some(res)
            } else {
                img.is_flushed = FlushType::StagingBufferFlushed;
                None
            };

            Ok(res)
        } else {
            Err(anyhow!("Img memory did not exist"))
        }
    }

    pub fn flush_buffer_memory(
        &mut self,
        mem: *mut u8,
        full_flush: bool,
    ) -> anyhow::Result<Option<(vk::Fence, vk::CommandBuffer, ash::Device)>> {
        if let Some(buffer) = self
            .ptr_work
            .mapped_memory_cache
            .get_mut(&std::ptr::NonNull::new(mem).unwrap())
        {
            // flush the staging buffer
            let upload_range = get_memory_range(
                &buffer.staging.buffer_mem,
                &buffer.staging.heap_data,
                &self.limits,
            );
            unsafe {
                self.device
                    .device
                    .flush_mapped_memory_ranges(&[upload_range])
                    .unwrap();
            }

            let res = if full_flush {
                Self::start_command_buffer(&self.device, &self.local.command_buffers)?;

                complete_buffer_object(
                    &self.device,
                    self.local.command_buffers.command_buffers[0],
                    &buffer.staging,
                    &buffer.device,
                    buffer.device.heap_data.allocation_size as vk::DeviceSize,
                )?;

                let res = Self::execute_command_buffer(
                    &self.device,
                    &self.fence,
                    &self.local.command_buffers,
                    &self.queue,
                )?;

                buffer.is_flushed = FlushType::FullyCreated;
                Some(res)
            } else {
                buffer.is_flushed = FlushType::StagingBufferFlushed;
                None
            };

            Ok(res)
        } else {
            Err(anyhow!("Buffer memory did not exist"))
        }
    }

    pub fn try_flush_mem(
        &mut self,
        mem: &mut GraphicsBackendMemory,
        do_expensive_flushing: bool,
    ) -> anyhow::Result<Option<(vk::Fence, vk::CommandBuffer, ash::Device)>> {
        match mem {
            GraphicsBackendMemory::Static(mem) => {
                let ptr = mem.mem.as_mut().unwrap().as_mut_ptr();
                if self.mem_block_image_exists(ptr) {
                    self.flush_img_memory(ptr, do_expensive_flushing)
                } else if self.mem_blocke_exists(ptr) {
                    self.flush_buffer_memory(ptr, do_expensive_flushing)
                } else {
                    Err(anyhow!("memory was not allocated."))
                }
            }
            GraphicsBackendMemory::Vector(_) => Err(anyhow!("tried to flush non driver memory")),
        }
    }

    // getters
    pub fn get_and_remove_mem_block(
        &mut self,
        mem: *mut u8,
    ) -> anyhow::Result<VulkanAllocatorBufferCacheEntry, ()> {
        let res = self
            .ptr_work
            .mapped_memory_cache
            .remove(&std::ptr::NonNull::new(mem).unwrap());
        if let Some(entry) = res {
            Ok(entry)
        } else {
            Err(())
        }
    }

    pub fn get_and_remove_mem_block_image(
        &mut self,
        mem: *mut u8,
    ) -> anyhow::Result<VulkanAllocatorImageCacheEntry, ()> {
        let res = self
            .ptr_work
            .mapped_memory_cache_image
            .remove(&std::ptr::NonNull::new(mem).unwrap());
        if let Some(entry) = res {
            Ok(entry)
        } else {
            Err(())
        }
    }

    pub fn mem_blocke_exists(&self, mem: *mut u8) -> bool {
        let res = self
            .ptr_work
            .mapped_memory_cache
            .get(&std::ptr::NonNull::new(mem).unwrap());
        if let Some(_) = res {
            true
        } else {
            false
        }
    }

    pub fn mem_block_image_exists(&self, mem: *mut u8) -> bool {
        let res = self
            .ptr_work
            .mapped_memory_cache_image
            .get(&std::ptr::NonNull::new(mem).unwrap());
        if let Some(_) = res {
            true
        } else {
            false
        }
    }

    pub fn mem_image_cache_entry(&self, mem: *mut u8) -> VulkanAllocatorImageCacheEntryData {
        let res = self
            .ptr_work
            .mapped_memory_cache_image
            .get(&std::ptr::NonNull::new(mem).unwrap())
            .unwrap();
        res.data
    }
}
