use std::{
    collections::{BTreeMap, HashMap},
    rc::Rc,
    sync::Arc,
};

use anyhow::anyhow;
use ash::vk;
use graphics_types::{
    commands::TexFlags,
    types::{GraphicsBackendMemory, GraphicsMemoryAllocationType},
};
use hiarc::Hiarc;
use libc::c_void;

use crate::backends::vulkan::utils::{complete_buffer_object, get_memory_range};

use super::{
    buffer::Buffer,
    command_buffer::CommandBuffers,
    command_pool::CommandPool,
    common::image_mip_level_count,
    descriptor_layout::DescriptorSetLayout,
    descriptor_pool::DescriptorPool,
    descriptor_set::DescriptorSets,
    fence::Fence,
    frame_resources::{FrameResources, RenderThreadFrameResources},
    image::Image,
    logical_device::LogicalDevice,
    memory::{MemoryBlock, MemoryCache, MemoryImageBlock},
    queue::Queue,
    utils::complete_texture,
    vulkan_device::Device,
    vulkan_limits::Limits,
    vulkan_mem::{AllocationError, BufferAllocationError, ImageAllocationError, Memory},
    vulkan_types::{DescriptorPoolType, DeviceDescriptorPools},
};

// good approximation of 1024x1024 image with mipmaps
pub const IMG_SIZE1024X1024: i64 = (1024 * 1024 * 4) * 2;

#[derive(Debug, Hiarc)]
pub enum FlushType {
    None,
    StagingBufferFlushed,
    FullyCreated,
}

#[derive(Debug, Hiarc)]
pub struct VulkanAllocatorBufferCacheEntry {
    pub staging: Arc<MemoryBlock>,
    pub device: Arc<MemoryBlock>,

    pub is_flushed: FlushType,
}

#[derive(Debug, Hiarc, Clone, Copy)]
pub struct VulkanAllocatorImageCacheEntryData {
    pub width: usize,
    pub height: usize,
    pub depth: usize,
    pub is_3d_tex: bool,
    pub flags: TexFlags,
    pub mip_map_count: usize,
}

#[derive(Debug, Hiarc)]
pub struct VulkanAllocatorImageCacheEntry {
    pub staging: Arc<MemoryBlock>,
    pub img: Arc<Image>,
    pub img_mem: MemoryImageBlock,

    pub data: VulkanAllocatorImageCacheEntryData,

    pub is_flushed: FlushType,
}

pub struct VulkanDeviceInternalMemory {
    pub(crate) mem: &'static mut [u8],
}

#[derive(Debug, Hiarc, Default)]
pub struct VulkanAllocatorPointerWork {
    mapped_memory_cache: HashMap<std::ptr::NonNull<u8>, VulkanAllocatorBufferCacheEntry>,
    mapped_memory_cache_image: HashMap<std::ptr::NonNull<u8>, VulkanAllocatorImageCacheEntry>,
}

unsafe impl Send for VulkanAllocatorPointerWork {}
unsafe impl Sync for VulkanAllocatorPointerWork {}

#[derive(Debug, Hiarc)]
pub struct VulkanAllocatorLocalData {
    command_buffers: Rc<CommandBuffers>,
}

unsafe impl Send for VulkanAllocatorLocalData {}
unsafe impl Sync for VulkanAllocatorLocalData {}

/// The vulkan allocator struct is specifically designed to be
/// used in a multi threaded scenario outside of the backend
#[derive(Debug, Hiarc)]
pub struct VulkanAllocator {
    pub mem: Memory,
    pub staging_buffer_cache: Arc<spin::Mutex<MemoryCache>>,
    pub staging_buffer_cache_image: Arc<spin::Mutex<MemoryCache>>,
    pub vertex_buffer_cache: Arc<spin::Mutex<MemoryCache>>,
    pub image_buffer_caches: BTreeMap<u32, Arc<spin::Mutex<MemoryCache>>>,

    pub limits: Limits,

    // private
    device: Arc<LogicalDevice>,
    ptr_work: VulkanAllocatorPointerWork,
    queue: Arc<Queue>,

    local: VulkanAllocatorLocalData,
    fence: Arc<Fence>,
}

impl VulkanAllocator {
    pub fn new(
        logical_device: Arc<LogicalDevice>,
        mem: Memory,
        limits: Limits,
        graphics_queue: Arc<Queue>,
    ) -> anyhow::Result<Arc<parking_lot::Mutex<Self>>> {
        let command_pool = CommandPool::new(
            logical_device.clone(),
            logical_device.phy_device.queue_node_index,
            1,
            0,
        )?;
        let command_buffers =
            CommandBuffers::new(command_pool.clone(), vk::CommandBufferLevel::PRIMARY, 1)?;
        let fence = Fence::new(logical_device.clone())?;
        Ok(Arc::new(parking_lot::Mutex::new(Self {
            device: logical_device,
            mem,
            staging_buffer_cache: MemoryCache::new(),
            staging_buffer_cache_image: MemoryCache::new(),
            image_buffer_caches: Default::default(),
            vertex_buffer_cache: MemoryCache::new(),
            limits,

            ptr_work: Default::default(),
            queue: graphics_queue,

            local: VulkanAllocatorLocalData { command_buffers },
            fence,
        })))
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
                        "memory block was not of correct type or was not found (requested type: {:?}), texture mem exists: {}, buffer mem exists: {}",
                        usage,
                        self.mem_block_image_exists(mem.as_ptr() as *mut _),
                        self.mem_blocke_exists(mem.as_ptr() as *mut _)
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

    pub fn destroy_caches(&mut self) {
        self.image_buffer_caches.clear();
    }

    pub fn get_image_memory(
        &mut self,
        required_size: vk::DeviceSize,
        required_alignment: vk::DeviceSize,
        required_memory_type_bits: u32,
    ) -> anyhow::Result<MemoryImageBlock, BufferAllocationError> {
        let it = self.image_buffer_caches.get_mut(&required_memory_type_bits);
        let mem: &Arc<spin::Mutex<MemoryCache>>;
        match it {
            None => {
                self.image_buffer_caches
                    .insert(required_memory_type_bits, MemoryCache::new());

                mem = self
                    .image_buffer_caches
                    .get_mut(&required_memory_type_bits)
                    .unwrap();
            }
            Some(it) => {
                mem = it;
            }
        }
        self.mem
            .get_image_memory_block_impl::<IMG_SIZE1024X1024, 2>(
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
    ) -> anyhow::Result<Arc<MemoryBlock>, BufferAllocationError> {
        self.mem
            .get_buffer_block_impl::<{ 8 * 1024 * 1024 }, 3, false>(
                &self.vertex_buffer_cache,
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
    ) -> anyhow::Result<(Arc<Image>, MemoryImageBlock), ImageAllocationError> {
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
            Device::get_sample_count(sample_count, &self.limits)
        } else {
            vk::SampleCountFlags::TYPE_1
        };
        image_info.sharing_mode = vk::SharingMode::EXCLUSIVE;

        let image = Image::new(self.device.clone(), image_info)?;

        let mem_requirements = image.get_image_memory_requirements();

        let image_memory = self.get_image_memory(
            mem_requirements.size,
            mem_requirements.alignment,
            mem_requirements.memory_type_bits,
        )?;

        image.bind(
            image_memory
                .base
                .buffer_mem(&mut FrameResources::new(None))
                .clone(),
            image_memory.base.heap_data.offset_to_align as u64,
        )?;

        Ok((image, image_memory))
    }

    pub fn get_staging_buffer(
        &mut self,
        buffer_data: *const c_void,
        required_size: vk::DeviceSize,
    ) -> anyhow::Result<Arc<MemoryBlock>, BufferAllocationError> {
        self.mem
            .get_buffer_block_impl::<{ 8 * 1024 * 1024 }, 3, true>(
                &self.staging_buffer_cache,
                vk::BufferUsageFlags::TRANSFER_SRC,
                vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_CACHED,
                buffer_data,
                required_size,
                std::cmp::max::<vk::DeviceSize>(self.limits.non_coherent_mem_alignment, 16),
            )
    }

    pub fn allocate_descriptor_pool(
        device: &Arc<LogicalDevice>,
        descriptor_pools: &mut DeviceDescriptorPools,
        alloc_pool_size: usize,
    ) -> anyhow::Result<()> {
        let mut pool_size = vk::DescriptorPoolSize::default();
        match descriptor_pools.pool_ty {
            DescriptorPoolType::CombineImgageAndSampler => {
                pool_size.ty = vk::DescriptorType::COMBINED_IMAGE_SAMPLER
            }
            DescriptorPoolType::Image => pool_size.ty = vk::DescriptorType::SAMPLED_IMAGE,
            DescriptorPoolType::Sampler => pool_size.ty = vk::DescriptorType::SAMPLER,
            DescriptorPoolType::Uniform => pool_size.ty = vk::DescriptorType::UNIFORM_BUFFER,
        }
        pool_size.descriptor_count = alloc_pool_size as u32;

        let pool_size = [pool_size];
        let mut pool_info = vk::DescriptorPoolCreateInfo::default().pool_sizes(&pool_size);
        pool_info.max_sets = alloc_pool_size as u32;
        pool_info.flags = vk::DescriptorPoolCreateFlags::FREE_DESCRIPTOR_SET;

        let res = DescriptorPool::new(device.clone(), &pool_info);
        if res.is_err() {
            return Err(anyhow!("Creating the descriptor pool failed."));
        }

        descriptor_pools.pools.push(res.unwrap());

        Ok(())
    }

    pub fn get_descriptor_pool_for_alloc(
        device: &Arc<LogicalDevice>,
        descriptor_pools: &Arc<parking_lot::Mutex<DeviceDescriptorPools>>,
        set_count: usize,
        layout: &Arc<DescriptorSetLayout>,
    ) -> anyhow::Result<Vec<Arc<DescriptorSets>>> {
        let mut cur_alloc_num = set_count;

        let mut pool_index_offset = 0;

        let mut res: Vec<Arc<DescriptorSets>> = Vec::new();

        let mut descriptor_pools = descriptor_pools.lock();

        while cur_alloc_num > 0 {
            let mut allocated_in_this_run = 0;

            let mut found = false;
            let mut descriptor_pool_index = usize::MAX;
            for i in pool_index_offset..descriptor_pools.pools.len() {
                let pool = &descriptor_pools.pools[i];
                if pool.cur_size.load(std::sync::atomic::Ordering::SeqCst) + (cur_alloc_num as u64)
                    < pool.size
                {
                    allocated_in_this_run = cur_alloc_num;
                    found = true;
                    descriptor_pool_index = i;
                    break;
                } else {
                    let remaining_pool_count =
                        pool.size - pool.cur_size.load(std::sync::atomic::Ordering::SeqCst);
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

                let default_alloc_size = descriptor_pools.default_alloc_size as usize;
                Self::allocate_descriptor_pool(device, &mut descriptor_pools, default_alloc_size)?;

                allocated_in_this_run =
                    std::cmp::min(descriptor_pools.default_alloc_size as usize, cur_alloc_num);
            }

            let pool = descriptor_pools.pools[descriptor_pool_index].clone();

            let new_descr = DescriptorSets::new(pool, allocated_in_this_run, layout)?;
            res.push(new_descr);

            pool_index_offset = descriptor_pool_index + 1;

            cur_alloc_num -= allocated_in_this_run;
        }

        Ok(res)
    }

    pub fn create_uniform_descriptor_sets(
        device: &Arc<LogicalDevice>,
        descriptor_pools: &Arc<parking_lot::Mutex<DeviceDescriptorPools>>,
        set_layout: &Arc<DescriptorSetLayout>,
        set_count: usize,
        bind_buffer: &Arc<Buffer>,
        single_buffer_instance_size: usize,
        memory_offset: vk::DeviceSize,
    ) -> anyhow::Result<Vec<Arc<DescriptorSets>>> {
        let descriptors =
            Self::get_descriptor_pool_for_alloc(device, descriptor_pools, set_count, set_layout)?;
        let mut cur_offset = 0;
        for i in 0..descriptors.len() {
            let set_count = descriptors[i].assign_uniform_buffer_to_sets(
                bind_buffer,
                memory_offset + cur_offset,
                single_buffer_instance_size as vk::DeviceSize,
            );

            cur_offset += (single_buffer_instance_size * set_count) as vk::DeviceSize
        }

        Ok(descriptors)
    }

    pub fn create_uniform_descriptor_set_layout(
        device: &Arc<LogicalDevice>,
        stage_flags: vk::ShaderStageFlags,
    ) -> anyhow::Result<Arc<DescriptorSetLayout>> {
        let mut sampler_layout_binding = vk::DescriptorSetLayoutBinding::default();
        sampler_layout_binding.binding = 0;
        sampler_layout_binding.descriptor_count = 1;
        sampler_layout_binding.descriptor_type = vk::DescriptorType::UNIFORM_BUFFER;
        sampler_layout_binding.stage_flags = stage_flags;

        let bindings = [sampler_layout_binding];
        let layout_info = vk::DescriptorSetLayoutCreateInfo::default().bindings(&bindings);

        DescriptorSetLayout::new(device.clone(), &layout_info)
    }

    pub fn get_staging_buffer_image(
        &mut self,
        mem: &Memory,
        limits: &Limits,
        buffer_data: &[u8],
        required_size: vk::DeviceSize,
    ) -> anyhow::Result<Arc<MemoryBlock>, BufferAllocationError> {
        mem.get_buffer_block_impl::<{ 8 * 1024 * 1024 }, 3, true>(
            &self.staging_buffer_cache_image,
            vk::BufferUsageFlags::TRANSFER_SRC,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_CACHED,
            buffer_data.as_ptr() as *const c_void,
            required_size,
            std::cmp::max::<vk::DeviceSize>(
                limits.optimal_image_copy_mem_alignment,
                std::cmp::max::<vk::DeviceSize>(limits.non_coherent_mem_alignment, 16),
            ),
        )
    }

    pub fn get_staging_buffer_for_mem_alloc(
        &mut self,
        buffer_data: *const c_void,
        required_size: vk::DeviceSize,
    ) -> anyhow::Result<&'static mut [u8], BufferAllocationError> {
        let res_block = self
            .mem
            .get_buffer_block_impl::<{ 8 * 1024 * 1024 }, 3, true>(
                &self.staging_buffer_cache,
                vk::BufferUsageFlags::TRANSFER_SRC,
                vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_CACHED,
                buffer_data,
                required_size,
                std::cmp::max::<vk::DeviceSize>(self.limits.non_coherent_mem_alignment, 16),
            )?;

        let res_buffer = self.get_vertex_buffer(required_size)?;

        let res = unsafe {
            let mem = res_block.mapped_buffer.as_ref().unwrap();
            mem.get_mem(required_size as usize)
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

        let res_block = self
            .mem
            .get_buffer_block_impl::<{ 8 * 1024 * 1024 }, 3, true>(
                &self.staging_buffer_cache_image,
                vk::BufferUsageFlags::TRANSFER_SRC,
                vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_CACHED,
                buffer_data,
                (width * height * depth * 4) as vk::DeviceSize,
                std::cmp::max::<vk::DeviceSize>(
                    self.limits.optimal_image_copy_mem_alignment,
                    std::cmp::max::<vk::DeviceSize>(self.limits.non_coherent_mem_alignment, 16),
                ),
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
        )?;

        let res = unsafe {
            let mem = res_block.mapped_buffer.as_ref().unwrap();
            mem.get_mem(width * height * depth * 4)
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
            device.device.begin_command_buffer(
                command_buffers.get(&mut RenderThreadFrameResources::new(None)),
                &begin_info,
            )
        }?;

        Ok(())
    }

    fn execute_command_buffer(
        device: &Arc<LogicalDevice>,
        fence: &Fence,
        command_buffers: &Rc<CommandBuffers>,
        queue: &Arc<Queue>,
    ) -> anyhow::Result<(vk::Fence, vk::CommandBuffer, ash::Device)> {
        unsafe {
            device.device.end_command_buffer(
                command_buffers.get(&mut RenderThreadFrameResources::new(None)),
            )?;
        }

        let command_buffers = [command_buffers.get(&mut RenderThreadFrameResources::new(None))];
        let submit_info = vk::SubmitInfo::default().command_buffers(&command_buffers);
        unsafe {
            device.device.reset_fences(&[fence.fence])?;
            let queue = queue.queues.lock();
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
                &mut FrameResources::new(None),
                img.staging.buffer_mem(&mut FrameResources::new(None)),
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

                let mut frame_resources = FrameResources::new(None);
                complete_texture(
                    &mut frame_resources,
                    &self.device,
                    self.local
                        .command_buffers
                        .get(&mut RenderThreadFrameResources::new(None)),
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
                &mut FrameResources::new(None),
                buffer.staging.buffer_mem(&mut FrameResources::new(None)),
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

                // can create
                let mut frame_resources = FrameResources::new(None);
                complete_buffer_object(
                    &mut frame_resources,
                    &self.device,
                    self.local
                        .command_buffers
                        .get(&mut RenderThreadFrameResources::new(None)),
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
