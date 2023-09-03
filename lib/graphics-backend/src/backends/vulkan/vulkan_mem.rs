use std::sync::{
    atomic::{AtomicU64, AtomicU8},
    Arc,
};

use ash::vk;
use libc::c_void;
use thiserror::Error;

use super::{
    common::{localizable, verbose_allocated_memory, verbose_deallocated_memory, EGFXErrorType},
    vulkan_dbg::is_verbose,
    vulkan_error::Error,
    vulkan_types::{
        EMemoryBlockUsage, SDeviceMemoryBlock, SMemoryBlock, SMemoryBlockCache, SMemoryCacheHeap,
        SMemoryHeapQueueElement, SMemoryImageBlock,
    },
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
}

pub struct Memory {
    dbg: Arc<AtomicU8>, // @see EDebugGFXModes

    instance: ash::Instance,
    pub device: ash::Device,
    vk_gpu: vk::PhysicalDevice,

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

        instance: &ash::Instance,
        device: &ash::Device,
        vk_gpu: vk::PhysicalDevice,

        texture_memory_usage: Arc<AtomicU64>,
        buffer_memory_usage: Arc<AtomicU64>,
        stream_memory_usage: Arc<AtomicU64>,
        staging_memory_usage: Arc<AtomicU64>,
    ) -> Self {
        Self {
            dbg: dbg,
            instance: instance.clone(),
            device: device.clone(),
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

        return 0;
    }

    #[must_use]
    fn allocate_vulkan_memory(
        &self,
        allocate_info: &vk::MemoryAllocateInfo,
    ) -> anyhow::Result<vk::DeviceMemory, MemoryAllocationError> {
        let res = unsafe { self.device.allocate_memory(allocate_info, None) };
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

    #[must_use]
    fn get_buffer_impl(
        &self,
        required_size: vk::DeviceSize,
        mem_usage: EMemoryBlockUsage,
        buffer_usage: vk::BufferUsageFlags,
        buffer_properties: vk::MemoryPropertyFlags,
        cur_image_index: u32,
    ) -> anyhow::Result<(vk::Buffer, SDeviceMemoryBlock), BufferAllocationError> {
        self.create_buffer(
            required_size,
            mem_usage,
            buffer_usage,
            buffer_properties,
            cur_image_index,
        )
    }

    #[must_use]
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
        )
            -> anyhow::Result<(vk::Buffer, SDeviceMemoryBlock), BufferAllocationError>,
    {
        let mut res_block: SMemoryBlock<ID> = Default::default();

        // if the required size is in the region of a single memory block
        // try to find it or create it
        if required_size < MEMORY_BLOCK_SIZE as vk::DeviceSize {
            let create_or_find_cache_block = || {
                let mut found_allocation = false;
                let mut allocated_mem = SMemoryHeapQueueElement::default();
                let mut tmp_block_memory = SDeviceMemoryBlock::default();
                let mut cache_heap: Option<&mut SMemoryCacheHeap>;
                let heaps = &mut memory_cache.memory_caches.memory_heaps;
                let mut found_index = 0;
                // try to allocate the memory inside existing heaps
                for i in 0..heaps.len() {
                    let heap = &mut heaps[i];

                    if (*heap).heap.allocate(
                        required_size as usize,
                        target_alignment as usize,
                        &mut allocated_mem,
                    ) {
                        tmp_block_memory = (*heap).buffer_mem.clone();
                        found_allocation = true;
                        found_index = i;
                        break;
                    }
                }
                // if no heap was suited, we create a new block
                if !found_allocation {
                    let mut new_heap = Box::new(SMemoryCacheHeap::default());

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
                    let tmp_buffer;
                    (tmp_buffer, tmp_block_memory) = buffer_allocation.unwrap();

                    let mut mapped_data_as_ptr: Option<&'static mut [u8]> = None;

                    if REQUIRES_MAPPING {
                        let unmapped = unsafe {
                            self.device.map_memory(
                                tmp_block_memory.mem,
                                0,
                                vk::WHOLE_SIZE,
                                vk::MemoryMapFlags::empty(),
                            )
                        };
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
                                mapped_data_as_ptr = Some(unsafe {
                                    std::slice::from_raw_parts_mut(
                                        mapped_mem as *mut u8,
                                        block_allocation_size as usize,
                                    )
                                });
                                true
                            }
                        } {
                            return Err(BufferAllocationError::BindMemoryToBufferFailed);
                        }
                    }

                    (*new_heap).buffer = tmp_buffer;

                    (*new_heap).buffer_mem = tmp_block_memory.clone();
                    (*new_heap).mapped_buffer = match mapped_data_as_ptr {
                        Some(data) => data.as_ptr() as *mut c_void,
                        None => std::ptr::null_mut(),
                    };

                    heaps.push(new_heap);
                    cache_heap = Some(heaps.last_mut().unwrap().as_mut());
                    cache_heap
                        .as_mut()
                        .unwrap()
                        .heap
                        .init(MEMORY_BLOCK_SIZE as usize * BLOCK_COUNT, 0);
                    if !cache_heap.as_mut().unwrap().heap.allocate(
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

                res_block.buffer = cache_heap.as_mut().unwrap().buffer;
                res_block.buffer_mem = tmp_block_memory.clone();
                if REQUIRES_MAPPING {
                    res_block.mapped_buffer = unsafe {
                        (cache_heap.as_mut().unwrap().mapped_buffer as *mut u8)
                            .offset(allocated_mem.offset_to_align as isize)
                            as *mut c_void
                    }
                } else {
                    res_block.mapped_buffer = std::ptr::null_mut();
                }
                res_block.is_cached = true;
                res_block.heap = &mut cache_heap.unwrap().heap;
                res_block.heap_data = allocated_mem;
                res_block.used_size = required_size;

                if REQUIRES_MAPPING {
                    if !buffer_data.is_null() {
                        unsafe {
                            libc::memcpy(
                                res_block.mapped_buffer,
                                buffer_data,
                                required_size as usize,
                            );
                        }
                    }
                }
                return Ok(res_block);
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

            let mut mapped_data = std::ptr::null_mut();
            if REQUIRES_MAPPING {
                unsafe {
                    mapped_data = self
                        .device
                        .map_memory(
                            tmp_block_memory.mem,
                            0,
                            vk::WHOLE_SIZE,
                            vk::MemoryMapFlags::empty(),
                        )
                        .unwrap();
                    if !buffer_data.is_null() {
                        libc::memcpy(mapped_data, buffer_data, required_size as usize);
                    }
                }
            }

            res_block.buffer = tmp_buffer;
            res_block.buffer_mem = tmp_block_memory;
            res_block.mapped_buffer = mapped_data;
            res_block.heap = std::ptr::null_mut();
            res_block.is_cached = false;
            res_block.heap_data.offset_to_align = 0;
            res_block.heap_data.allocation_size = required_size as usize;
            res_block.used_size = required_size;

            Ok(res_block)
        }
    }

    #[must_use]
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
        cur_image_index: u32,
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
                    let buffer_allocation = self.get_buffer_impl(
                        required_size,
                        mem_usage,
                        buffer_usage,
                        buffer_properties,
                        cur_image_index
                    ) ;
                    Ok(buffer_allocation?)
                },
                0,
            )
    }

    #[must_use]
    pub fn create_buffer(
        &self,
        buffer_size: vk::DeviceSize,
        mem_usage: EMemoryBlockUsage,
        buffer_usage: vk::BufferUsageFlags,
        memory_properties: vk::MemoryPropertyFlags,

        cur_image_index: u32,
    ) -> anyhow::Result<(vk::Buffer, SDeviceMemoryBlock), BufferAllocationError> {
        let mut buffer_info = vk::BufferCreateInfo::default();
        buffer_info.size = buffer_size;
        buffer_info.usage = buffer_usage;
        buffer_info.sharing_mode = vk::SharingMode::EXCLUSIVE;

        let created_buffer_res = unsafe { self.device.create_buffer(&buffer_info, None) };
        if let Err(_) = created_buffer_res {
            self.error.lock().unwrap().set_error(
                EGFXErrorType::OutOfMemoryBuffer,
                localizable("Buffer creation failed."),
            );
            return Err(BufferAllocationError::BufferCreationFailed);
        }
        let vk_buffer = created_buffer_res.unwrap();

        let mem_requirements: vk::MemoryRequirements;
        mem_requirements = unsafe { self.device.get_buffer_memory_requirements(vk_buffer) };

        let mut mem_alloc_info = vk::MemoryAllocateInfo::default();
        mem_alloc_info.allocation_size = mem_requirements.size;
        mem_alloc_info.memory_type_index = self.find_memory_type(
            self.vk_gpu,
            mem_requirements.memory_type_bits,
            memory_properties,
        );

        let mut vk_buffer_memory: SDeviceMemoryBlock = Default::default();
        vk_buffer_memory.size = mem_requirements.size;

        let allocation = self.allocate_vulkan_memory(&mem_alloc_info);
        if let Err(err) = &allocation {
            self.error.lock().unwrap().set_error(
                EGFXErrorType::OutOfMemoryBuffer,
                localizable("Allocation for buffer object failed."),
            );

            unsafe { self.device.destroy_buffer(vk_buffer, None) };
            return Err(BufferAllocationError::MemoryAllocationError(*err));
        }
        vk_buffer_memory.mem = allocation.unwrap();

        vk_buffer_memory.usage_type = mem_usage;

        let res = unsafe {
            self.device
                .bind_buffer_memory(vk_buffer, vk_buffer_memory.mem, 0)
        };
        if res.is_err() {
            self.error.lock().unwrap().set_error(
                EGFXErrorType::OutOfMemoryBuffer,
                localizable("Binding memory to buffer failed."),
            );

            unsafe {
                self.device.destroy_buffer(vk_buffer, None);
                self.device.free_memory(vk_buffer_memory.mem, None);
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
            verbose_allocated_memory(
                mem_requirements.size,
                cur_image_index as usize,
                mem_usage.clone(),
            );
        }

        Ok((vk_buffer, vk_buffer_memory))
    }

    #[must_use]
    fn get_image_memory_impl(
        &self,
        required_size: vk::DeviceSize,
        required_memory_type_bits: u32,

        buffer_properties: vk::MemoryPropertyFlags,
    ) -> anyhow::Result<SDeviceMemoryBlock, BufferAllocationError> {
        let mut mem_alloc_info = vk::MemoryAllocateInfo::default();
        mem_alloc_info.allocation_size = required_size;
        mem_alloc_info.memory_type_index =
            self.find_memory_type(self.vk_gpu, required_memory_type_bits, buffer_properties);

        let mut buffer_memory: SDeviceMemoryBlock = Default::default();
        buffer_memory.size = required_size;

        let allocation = self.allocate_vulkan_memory(&mem_alloc_info);
        if let Err(err) = &allocation {
            self.error.lock().unwrap().set_error(
                EGFXErrorType::OutOfMemoryImage,
                localizable("Allocation for image memory failed."),
            );
            return Err(BufferAllocationError::MemoryAllocationError(*err));
        }
        buffer_memory.mem = allocation.unwrap();

        self.texture_memory_usage
            .fetch_sub(required_size, std::sync::atomic::Ordering::Relaxed);

        if is_verbose(&*self.dbg) {
            // TODO!!! self.VerboseAllocatedMemory(RequiredSize, self.m_CurImageIndex as usize, EMemoryBlockUsage::MEMORY_BLOCK_USAGE_TEXTURE);
        }

        buffer_memory.usage_type = EMemoryBlockUsage::Texture;

        Ok(buffer_memory)
    }

    #[must_use]
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
        let mut result_block: SMemoryImageBlock<ID> = Default::default();
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

        result_block.base = base_block;
        result_block.image_memory_bits = required_memory_type_bits;

        Ok(result_block)
    }

    pub fn clean_buffer_pair(
        &self,
        image_index: usize,
        buffer: &mut vk::Buffer,
        buffer_mem: &mut SDeviceMemoryBlock,
    ) {
        let is_buffer: bool = *buffer != vk::Buffer::null();
        if is_buffer {
            unsafe {
                self.device.destroy_buffer(*buffer, None);
            }

            *buffer = vk::Buffer::null();
        }
        if buffer_mem.mem != vk::DeviceMemory::null() {
            unsafe {
                self.device.free_memory(buffer_mem.mem, None);
            }
            if buffer_mem.usage_type == EMemoryBlockUsage::Buffer {
                self.buffer_memory_usage
                    .fetch_sub(buffer_mem.size, std::sync::atomic::Ordering::Relaxed);
            } else if buffer_mem.usage_type == EMemoryBlockUsage::Texture {
                self.texture_memory_usage
                    .fetch_sub(buffer_mem.size, std::sync::atomic::Ordering::Relaxed);
            } else if buffer_mem.usage_type == EMemoryBlockUsage::Stream {
                self.stream_memory_usage
                    .fetch_sub(buffer_mem.size, std::sync::atomic::Ordering::Relaxed);
            } else if buffer_mem.usage_type == EMemoryBlockUsage::Staging {
                self.staging_memory_usage
                    .fetch_sub(buffer_mem.size, std::sync::atomic::Ordering::Relaxed);
            }

            if is_verbose(&*self.dbg) {
                verbose_deallocated_memory(
                    buffer_mem.size,
                    image_index as usize,
                    buffer_mem.usage_type,
                );
            }

            buffer_mem.mem = vk::DeviceMemory::null();
        }
    }
}
