use std::sync::{
    atomic::{AtomicU64, AtomicU8},
    Arc,
};

use ash::vk;
use libc::c_void;
use thiserror::Error;

use super::{
    buffer::Buffer,
    common::{localizable, verbose_allocated_memory, EGFXErrorType},
    instance::Instance,
    logical_device::LogicalDevice,
    mapped_memory::MappedMemory,
    memory::{
        SMemoryBlock, SMemoryBlockCache, SMemoryHeapForVkMemory, SMemoryHeapQueueElement,
        SMemoryHeapType, SMemoryImageBlock,
    },
    memory_block::SDeviceMemoryBlock,
    phy_device::PhyDevice,
    vulkan_dbg::is_verbose,
    vulkan_error::Error,
    vulkan_types::EMemoryBlockUsage,
};

#[derive(Debug, Error, Copy, Clone)]
pub enum MemoryAllocationError {
    #[error("Host is out of memory.")]
    OutOfHostMem,
    #[error("Device is out of memory.")]
    OutOfDeviceMem,
    #[error("Not specifically handled vulkan result {0}.")]
    VkError(vk::Result),
}

#[derive(Debug, Error, Copy, Clone)]
pub enum BufferAllocationError {
    #[error("Buffer creation failed.")]
    BufferCreationFailed,
    #[error("Memory allocation failed: {0}")]
    MemoryAllocationError(MemoryAllocationError),
    #[error("Binding memory to buffer failed.")]
    BindMemoryToBufferFailed,
    #[error("Mapping raw memory failed.")]
    MapMemoryFailed,
    #[error("Heap allocation failed.")]
    HeapAllocationFailed,
    #[error("Memory related operation failed.")]
    MemoryRelatedOperationFailed,
}

impl From<MemoryAllocationError> for BufferAllocationError {
    fn from(value: MemoryAllocationError) -> Self {
        Self::MemoryAllocationError(value)
    }
}

impl From<vk::Result> for BufferAllocationError {
    fn from(_value: vk::Result) -> Self {
        Self::BufferCreationFailed
    }
}

#[derive(Debug, Error, Copy, Clone)]
pub enum ImageAllocationError {
    #[error("Image creation failed.")]
    ImageCreationFailed,
    #[error("Memory allocation failed: {0}")]
    MemoryAllocationError(MemoryAllocationError),
    #[error("Binding memory to image failed.")]
    BindMemoryToImageFailed,
    #[error("Mapping raw memory failed.")]
    MapMemoryFailed,
    #[error("Heap allocation failed.")]
    HeapAllocationFailed,
    #[error("Memory related operation failed.")]
    MemoryRelatedOperationFailed,
    #[error("Buffer allocation for copying failed.")]
    BufferAllocationError(BufferAllocationError),
    #[error("Image dimensions too big.")]
    ImageDimensionsTooBig,
}

impl From<MemoryAllocationError> for ImageAllocationError {
    fn from(value: MemoryAllocationError) -> Self {
        Self::MemoryAllocationError(value)
    }
}

impl From<vk::Result> for ImageAllocationError {
    fn from(_value: vk::Result) -> Self {
        Self::ImageCreationFailed
    }
}

impl From<BufferAllocationError> for ImageAllocationError {
    fn from(value: BufferAllocationError) -> Self {
        Self::BufferAllocationError(value)
    }
}

#[derive(Debug, Error, Copy, Clone)]
pub enum AllocationError {
    #[error("Image allocation error.")]
    ImageAllocationError(ImageAllocationError),
    #[error("Buffer allocation error.")]
    BufferAllocationError(BufferAllocationError),
    #[error("Memory allocation error.")]
    MemoryAllocationError(MemoryAllocationError),
}

impl From<ImageAllocationError> for AllocationError {
    fn from(value: ImageAllocationError) -> Self {
        Self::ImageAllocationError(value)
    }
}

impl From<BufferAllocationError> for AllocationError {
    fn from(value: BufferAllocationError) -> Self {
        Self::BufferAllocationError(value)
    }
}

impl From<MemoryAllocationError> for AllocationError {
    fn from(value: MemoryAllocationError) -> Self {
        Self::MemoryAllocationError(value)
    }
}

#[derive(Clone)]
pub struct Memory {
    dbg: Arc<AtomicU8>, // @see EDebugGFXModes

    instance: Arc<Instance>,
    pub logical_device: Arc<LogicalDevice>,
    vk_gpu: Arc<PhyDevice>,

    error: Arc<std::sync::Mutex<Error>>,

    texture_memory_usage: Arc<AtomicU64>,
    buffer_memory_usage: Arc<AtomicU64>,
    stream_memory_usage: Arc<AtomicU64>,
    staging_memory_usage: Arc<AtomicU64>,
}

impl std::fmt::Debug for Memory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Memory")
            .field("dbg", &self.dbg)
            .field("vk_gpu", &self.vk_gpu)
            .field("error", &self.error)
            .field("texture_memory_usage", &self.texture_memory_usage)
            .field("buffer_memory_usage", &self.buffer_memory_usage)
            .field("stream_memory_usage", &self.stream_memory_usage)
            .field("staging_memory_usage", &self.staging_memory_usage)
            .finish()
    }
}

impl Memory {
    pub fn new(
        dbg: Arc<AtomicU8>, // @see EDebugGFXModes
        error: Arc<std::sync::Mutex<Error>>,

        instance: Arc<Instance>,
        device: Arc<LogicalDevice>,
        vk_gpu: Arc<PhyDevice>,

        texture_memory_usage: Arc<AtomicU64>,
        buffer_memory_usage: Arc<AtomicU64>,
        stream_memory_usage: Arc<AtomicU64>,
        staging_memory_usage: Arc<AtomicU64>,
    ) -> Self {
        Self {
            dbg: dbg,
            instance: instance,
            logical_device: device.clone(),
            vk_gpu,
            error: error,
            texture_memory_usage,
            buffer_memory_usage,
            stream_memory_usage,
            staging_memory_usage,
        }
    }

    /************************
     * MEMORY MANAGEMENT
     ************************/
    pub fn find_memory_type(
        &self,
        phy_device: vk::PhysicalDevice,
        type_filter: u32,
        properties: vk::MemoryPropertyFlags,
    ) -> u32 {
        let mem_properties: vk::PhysicalDeviceMemoryProperties;
        mem_properties = unsafe {
            self.instance
                .vk_instance
                .get_physical_device_memory_properties(phy_device)
        };

        for i in 0..mem_properties.memory_type_count {
            if (type_filter & (1 << i)) != 0
                && (mem_properties.memory_types[i as usize].property_flags & properties)
                    == properties
            {
                return i;
            }
        }

        0
    }

    fn allocate_vulkan_memory(
        &self,
        allocate_info: vk::MemoryAllocateInfo,
        usage_type: EMemoryBlockUsage,
    ) -> anyhow::Result<Arc<SDeviceMemoryBlock>, MemoryAllocationError> {
        let res = SDeviceMemoryBlock::new(self.logical_device.clone(), allocate_info, usage_type);
        if let Err(err) = res {
            // TODO  dbg_msg("vulkan", "vulkan memory allocation failed, trying to recover.");
            match err {
                vk::Result::ERROR_OUT_OF_HOST_MEMORY => Err(MemoryAllocationError::OutOfHostMem),
                vk::Result::ERROR_OUT_OF_DEVICE_MEMORY => {
                    Err(MemoryAllocationError::OutOfDeviceMem)
                }
                _ => Err(MemoryAllocationError::VkError(err)),
            }
        } else {
            Ok(res.unwrap())
        }
    }

    pub fn get_block_impl<
        const ID: usize,
        const MEMORY_BLOCK_SIZE: i64,
        const BLOCK_COUNT: usize,
        const REQUIRES_MAPPING: bool,
        FAlloc,
    >(
        &self,
        memory_cache: &mut SMemoryBlockCache<ID>,
        buffer_usage: vk::BufferUsageFlags,
        buffer_properties: vk::MemoryPropertyFlags,
        buffer_data: *const c_void,
        required_size: vk::DeviceSize,
        target_alignment: vk::DeviceSize,
        allocate_block: FAlloc,
        memory_requirement_bits: u32,
    ) -> anyhow::Result<SMemoryBlock<ID>, BufferAllocationError>
    where
        FAlloc: FnOnce(
            vk::DeviceSize,
            EMemoryBlockUsage,
            vk::BufferUsageFlags,
            vk::MemoryPropertyFlags,
            u32, // memory requirement bits
        ) -> anyhow::Result<
            (Option<Arc<Buffer>>, Arc<SDeviceMemoryBlock>),
            BufferAllocationError,
        >,
    {
        // if the required size is in the region of a single memory block
        // try to find it or create it
        if required_size < MEMORY_BLOCK_SIZE as vk::DeviceSize {
            let create_or_find_cache_block = || {
                let mut found_allocation = false;
                let mut allocated_mem = SMemoryHeapQueueElement::default();
                let mut tmp_block_memory = None;
                let mut cache_heap: Option<&mut SMemoryHeapForVkMemory<ID>>;
                let heaps = &mut memory_cache.memory_caches.memory_heaps;
                let mut found_index = 0;
                // try to allocate the memory inside existing heaps
                for i in 0..heaps.len() {
                    let heap = &mut heaps[i];

                    if heap.heap.lock().allocate(
                        required_size as usize,
                        target_alignment as usize,
                        &mut allocated_mem,
                    ) {
                        tmp_block_memory = Some(heap.buffer_mem.clone());
                        found_allocation = true;
                        found_index = i;
                        break;
                    }
                }
                // if no heap was suited, we create a new block
                if !found_allocation {
                    let block_allocation_size = MEMORY_BLOCK_SIZE as u64 * BLOCK_COUNT as u64;
                    let buffer_allocation = allocate_block(
                        block_allocation_size,
                        if REQUIRES_MAPPING {
                            EMemoryBlockUsage::Staging
                        } else {
                            EMemoryBlockUsage::Buffer
                        },
                        buffer_usage,
                        buffer_properties,
                        memory_requirement_bits,
                    );
                    if let Err(err) = buffer_allocation {
                        return Err(err);
                    }

                    let (tmp_buffer, res_block) = buffer_allocation.unwrap();
                    tmp_block_memory = Some(res_block.clone());

                    let mut mapped_data_as_ptr: Option<(isize, Arc<MappedMemory>)> = None;

                    if REQUIRES_MAPPING {
                        let unmapped = MappedMemory::new(
                            self.logical_device.clone(),
                            tmp_block_memory.as_ref().unwrap().clone(),
                            0,
                        );
                        if !match unmapped {
                            Err(_) => {
                                // TODO: add case for image
                                self.error.lock().unwrap().set_error(
                                    if REQUIRES_MAPPING {
                                        EGFXErrorType::OutOfMemoryStaging
                                    } else {
                                        EGFXErrorType::OutOfMemoryBuffer
                                    },
                                    localizable("Failed to map buffer block memory."),
                                );
                                false
                            }
                            Ok(mapped_mem) => {
                                mapped_data_as_ptr = Some((0, mapped_mem));
                                true
                            }
                        } {
                            return Err(BufferAllocationError::BindMemoryToBufferFailed);
                        }
                    }

                    let new_heap = SMemoryHeapForVkMemory::new(
                        Arc::downgrade(&memory_cache.frame_delayed_cached_buffer_cleanups),
                        tmp_buffer,
                        res_block,
                        mapped_data_as_ptr,
                        MEMORY_BLOCK_SIZE as usize * BLOCK_COUNT,
                        0,
                    );
                    heaps.push(new_heap);
                    cache_heap = Some(heaps.last_mut().unwrap());
                    if !cache_heap.as_mut().unwrap().heap.lock().allocate(
                        required_size as usize,
                        target_alignment as usize,
                        &mut allocated_mem,
                    ) {
                        // TODO: add case for image
                        self.error.lock().unwrap().set_error(
                            if REQUIRES_MAPPING {
                                EGFXErrorType::OutOfMemoryStaging
                            } else {
                                EGFXErrorType::OutOfMemoryBuffer
                            },
                            localizable(
                                "Heap allocation failed directly after creating fresh heap.",
                            ),
                        );
                        return Err(BufferAllocationError::HeapAllocationFailed);
                    }
                } else {
                    let heap = &mut memory_cache.memory_caches.memory_heaps[found_index];
                    cache_heap = Some(&mut *heap);
                }

                let mut res_block = SMemoryBlock::<ID>::new(tmp_block_memory.unwrap());
                res_block.buffer = cache_heap.as_mut().unwrap().buffer.clone();
                if REQUIRES_MAPPING {
                    let mem = cache_heap.as_ref().unwrap().mapped_buffer.clone().unwrap();
                    res_block.mapped_buffer =
                        Some((mem.0 + allocated_mem.offset_to_align as isize, mem.1));
                } else {
                    res_block.mapped_buffer = None;
                }
                res_block.heap = SMemoryHeapType::Cached(cache_heap.cloned().unwrap());
                res_block.heap_data = allocated_mem;
                res_block
                    .used_size
                    .store(required_size, std::sync::atomic::Ordering::SeqCst);

                if REQUIRES_MAPPING {
                    if !buffer_data.is_null() {
                        let mem = res_block.mapped_buffer.as_ref().unwrap();
                        unsafe {
                            libc::memcpy(
                                mem.1.get_mem().offset(mem.0) as *mut _,
                                buffer_data,
                                required_size as usize,
                            );
                        }
                    }
                }
                Ok(res_block)
            };
            create_or_find_cache_block()
        } else {
            let block_allocation = allocate_block(
                required_size,
                if REQUIRES_MAPPING {
                    EMemoryBlockUsage::Staging
                } else {
                    EMemoryBlockUsage::Buffer
                },
                buffer_usage,
                buffer_properties,
                memory_requirement_bits,
            );
            if let Err(err) = block_allocation {
                return Err(err);
            }
            let (tmp_buffer, tmp_block_memory) = block_allocation.unwrap();

            let mut mapped_data = None;
            if REQUIRES_MAPPING {
                unsafe {
                    mapped_data = Some(
                        MappedMemory::new(self.logical_device.clone(), tmp_block_memory.clone(), 0)
                            .unwrap(),
                    );
                    if !buffer_data.is_null() {
                        libc::memcpy(
                            mapped_data.as_ref().unwrap().get_mem() as *mut _,
                            buffer_data,
                            required_size as usize,
                        );
                    }
                }
            }

            let mut res_block = SMemoryBlock::<ID>::new(tmp_block_memory);
            res_block.buffer = tmp_buffer;
            res_block.mapped_buffer = mapped_data.map(|i| (0, i));
            res_block.heap = SMemoryHeapType::None;
            res_block.heap_data.offset_to_align = 0;
            res_block.heap_data.allocation_size = required_size as usize;
            res_block
                .used_size
                .store(required_size, std::sync::atomic::Ordering::SeqCst);

            Ok(res_block)
        }
    }

    pub fn get_buffer_block_impl<
        const ID: usize,
        const MEMORY_BLOCK_SIZE: i64,
        const BLOCK_COUNT: usize,
        const REQUIRES_MAPPING: bool,
    >(
        &self,
        memory_cache: &mut SMemoryBlockCache<ID>,
        buffer_usage: vk::BufferUsageFlags,
        buffer_properties: vk::MemoryPropertyFlags,
        buffer_data: *const c_void,
        requized_size: vk::DeviceSize,
        target_alignment: vk::DeviceSize,
    ) -> anyhow::Result<SMemoryBlock<ID>, BufferAllocationError> {
        self
            .get_block_impl::<{ ID }, { MEMORY_BLOCK_SIZE }, { BLOCK_COUNT }, { REQUIRES_MAPPING }, _>(memory_cache,
                buffer_usage,
                buffer_properties,
                buffer_data,
                requized_size,
                target_alignment,
                |required_size: vk::DeviceSize,
                 mem_usage: EMemoryBlockUsage,
                 buffer_usage: vk::BufferUsageFlags,
                 buffer_properties: vk::MemoryPropertyFlags,
                 _| {
                    let (buffer, mem) = self.create_buffer(
                        required_size,
                        mem_usage,
                        buffer_usage,
                        buffer_properties,
                    ) ?;
                    Ok((Some(buffer), mem))
                },
                0,
            )
    }

    pub fn create_buffer(
        &self,
        buffer_size: vk::DeviceSize,
        mem_usage: EMemoryBlockUsage,
        buffer_usage: vk::BufferUsageFlags,
        memory_properties: vk::MemoryPropertyFlags,
    ) -> anyhow::Result<(Arc<Buffer>, Arc<SDeviceMemoryBlock>), BufferAllocationError> {
        let mut buffer_info = vk::BufferCreateInfo::default();
        buffer_info.size = buffer_size;
        buffer_info.usage = buffer_usage;
        buffer_info.sharing_mode = vk::SharingMode::EXCLUSIVE;

        let created_buffer_res = Buffer::new(self.logical_device.clone(), buffer_info);
        if let Err(_) = created_buffer_res {
            self.error.lock().unwrap().set_error(
                EGFXErrorType::OutOfMemoryBuffer,
                localizable("Buffer creation failed."),
            );
            return Err(BufferAllocationError::BufferCreationFailed);
        }
        let vk_buffer = created_buffer_res.unwrap();

        let mem_requirements: vk::MemoryRequirements;
        mem_requirements = unsafe {
            self.logical_device
                .device
                .get_buffer_memory_requirements(vk_buffer.buffer)
        };

        let mut mem_alloc_info = vk::MemoryAllocateInfo::default();
        mem_alloc_info.allocation_size = mem_requirements.size;
        mem_alloc_info.memory_type_index = self.find_memory_type(
            self.vk_gpu.cur_device,
            mem_requirements.memory_type_bits,
            memory_properties,
        );

        let allocation = self.allocate_vulkan_memory(mem_alloc_info, mem_usage);
        if let Err(err) = &allocation {
            self.error.lock().unwrap().set_error(
                EGFXErrorType::OutOfMemoryBuffer,
                localizable("Allocation for buffer object failed."),
            );

            return Err(BufferAllocationError::MemoryAllocationError(*err));
        }
        let vk_buffer_memory = allocation.unwrap();

        let res = vk_buffer.bind(vk_buffer_memory.clone());
        if res.is_err() {
            self.error.lock().unwrap().set_error(
                EGFXErrorType::OutOfMemoryBuffer,
                localizable("Binding memory to buffer failed."),
            );

            unsafe {
                self.logical_device
                    .device
                    .free_memory(vk_buffer_memory.mem, None);
            }
            return Err(BufferAllocationError::MapMemoryFailed);
        }

        if mem_usage == EMemoryBlockUsage::Buffer {
            self.buffer_memory_usage
                .fetch_add(mem_requirements.size, std::sync::atomic::Ordering::Relaxed);
        } else if mem_usage == EMemoryBlockUsage::Staging {
            self.staging_memory_usage
                .fetch_add(mem_requirements.size, std::sync::atomic::Ordering::Relaxed);
        } else if mem_usage == EMemoryBlockUsage::Stream {
            self.stream_memory_usage
                .fetch_add(mem_requirements.size, std::sync::atomic::Ordering::Relaxed);
        }

        if is_verbose(&*self.dbg) {
            verbose_allocated_memory(mem_requirements.size, mem_usage.clone());
        }

        Ok((vk_buffer, vk_buffer_memory))
    }

    fn get_image_memory_impl(
        &self,
        required_size: vk::DeviceSize,
        required_memory_type_bits: u32,

        buffer_properties: vk::MemoryPropertyFlags,
    ) -> anyhow::Result<Arc<SDeviceMemoryBlock>, BufferAllocationError> {
        let mut mem_alloc_info = vk::MemoryAllocateInfo::default();
        mem_alloc_info.allocation_size = required_size;
        mem_alloc_info.memory_type_index = self.find_memory_type(
            self.vk_gpu.cur_device,
            required_memory_type_bits,
            buffer_properties,
        );

        let allocation = self.allocate_vulkan_memory(mem_alloc_info, EMemoryBlockUsage::Texture);
        if let Err(err) = &allocation {
            self.error.lock().unwrap().set_error(
                EGFXErrorType::OutOfMemoryImage,
                localizable("Allocation for image memory failed."),
            );
            return Err(BufferAllocationError::MemoryAllocationError(*err));
        }
        let buffer_memory = allocation.unwrap();

        self.texture_memory_usage
            .fetch_sub(required_size, std::sync::atomic::Ordering::Relaxed);

        if is_verbose(&*self.dbg) {
            // TODO!!! self.VerboseAllocatedMemory(RequiredSize, self.m_CurImageIndex as usize, EMemoryBlockUsage::MEMORY_BLOCK_USAGE_TEXTURE);
        }

        Ok(buffer_memory)
    }

    pub fn get_image_memory_block_impl<
        const ID: usize,
        const MEMORY_BLOCK_SIZE: i64,
        const BLOCK_COUNT: usize,
    >(
        &mut self,
        memory_cache: &mut SMemoryBlockCache<ID>,
        buffer_properties: vk::MemoryPropertyFlags,
        required_size: vk::DeviceSize,
        required_alignment: vk::DeviceSize,
        required_memory_type_bits: u32,
    ) -> anyhow::Result<SMemoryImageBlock<ID>, BufferAllocationError> {
        let base_block = self
            .get_block_impl::<{ ID }, { MEMORY_BLOCK_SIZE }, { BLOCK_COUNT }, false, _>(
                memory_cache,
                vk::BufferUsageFlags::empty(),
                buffer_properties,
                std::ptr::null(),
                required_size,
                required_alignment,
                |required_size, _, _, buffer_properties, required_memory_type_bits| {
                    let memory_block = self.get_image_memory_impl(
                        required_size,
                        required_memory_type_bits,
                        buffer_properties,
                    )?;
                    Ok((Default::default(), memory_block))
                },
                0,
            )?;

        let result_block = SMemoryImageBlock::<ID> {
            base: base_block,
            image_memory_bits: required_memory_type_bits,
        };

        Ok(result_block)
    }
}
