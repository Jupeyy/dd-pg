use std::collections::HashMap;

use ash::vk;
use graphics_types::types::GraphicsBackendMemory;
use libc::c_void;

use super::{
    vulkan_limits::Limits,
    vulkan_mem::Memory,
    vulkan_types::{SDeviceMemoryBlock, SMemoryBlock, SMemoryBlockCache, SMemoryHeapQueueElement},
};

// these caches are designed to be used outside of the backend
pub const THREADED_STAGING_BUFFER_CACHE_ID: usize = 4;
pub const THREADED_STAGING_BUFFER_IMAGE_CACHE_ID: usize = 5;

pub struct VulkanAllocatorCacheEntry<const ID: usize> {
    block: SMemoryBlock<{ ID }>,
}

/**
 * The vulkan allocator struct is specifically designed to be
 * used in a multi threaded scenario outside of the backend
 */
pub struct VulkanAllocator {
    pub mem: Memory,
    pub staging_buffer_cache: SMemoryBlockCache<{ THREADED_STAGING_BUFFER_CACHE_ID }>,
    pub staging_buffer_cache_image: SMemoryBlockCache<{ THREADED_STAGING_BUFFER_IMAGE_CACHE_ID }>,
    pub limits: Limits,

    pub mapped_memory_cache: HashMap<
        std::ptr::NonNull<c_void>,
        VulkanAllocatorCacheEntry<THREADED_STAGING_BUFFER_CACHE_ID>,
    >,
    pub mapped_memory_cache_image: HashMap<
        std::ptr::NonNull<c_void>,
        VulkanAllocatorCacheEntry<THREADED_STAGING_BUFFER_IMAGE_CACHE_ID>,
    >,

    // if the memory was free'd in a frame, then it should use the Some(frame_index)
    // as key, else it should use None, indicating that the memory was free'd
    // outside of the backend
    pub cleanups: HashMap<Option<u32>, Vec<std::ptr::NonNull<c_void>>>,
}

unsafe impl Send for VulkanAllocator {}
unsafe impl Sync for VulkanAllocator {}

impl VulkanAllocator {
    pub fn new(mem: Memory, limits: Limits) -> Self {
        Self {
            mem,
            staging_buffer_cache: Default::default(),
            staging_buffer_cache_image: Default::default(),
            limits,

            mapped_memory_cache: Default::default(),
            mapped_memory_cache_image: Default::default(),
            cleanups: Default::default(),
        }
    }

    /************************
     * MEMORY MANAGEMENT
     ************************/
    #[must_use]
    pub fn get_staging_buffer(
        &mut self,
        res_block: &mut SMemoryBlock<THREADED_STAGING_BUFFER_CACHE_ID>,
        buffer_data: *const c_void,
        required_size: vk::DeviceSize,
    ) -> bool {
        let res = self
            .mem
            .get_buffer_block_impl::<{ THREADED_STAGING_BUFFER_CACHE_ID }, { 8 * 1024 * 1024 }, 3, true>(
                res_block,
                &mut self.staging_buffer_cache,
                vk::BufferUsageFlags::TRANSFER_SRC,
                vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_CACHED,
                buffer_data,
                required_size,
                std::cmp::max::<vk::DeviceSize>(self.limits.non_coherent_mem_alignment, 16),
            );

        if res {
            self.mapped_memory_cache.insert(
                std::ptr::NonNull::new(res_block.mapped_buffer).unwrap(),
                VulkanAllocatorCacheEntry {
                    block: res_block.clone(),
                },
            );
        }

        res
    }

    #[must_use]
    pub fn get_staging_buffer_image(
        &mut self,
        res_block: &mut SMemoryBlock<THREADED_STAGING_BUFFER_IMAGE_CACHE_ID>,
        buffer_data: *const c_void,
        required_size: vk::DeviceSize,
    ) -> bool {
        let res = self.mem
             .get_buffer_block_impl::<{ THREADED_STAGING_BUFFER_IMAGE_CACHE_ID }, { 8 * 1024 * 1024 }, 3, true>(
                 res_block,
                 &mut self.staging_buffer_cache_image,
                 vk::BufferUsageFlags::TRANSFER_SRC,
                 vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_CACHED,
                 buffer_data,
                 required_size,
                 std::cmp::max::<vk::DeviceSize>(
                     self.limits.optimal_image_copy_mem_alignment,
                     std::cmp::max::<vk::DeviceSize>(self.limits.non_coherent_mem_alignment, 16),
                 ),
             );

        if res {
            self.mapped_memory_cache_image.insert(
                std::ptr::NonNull::new(res_block.mapped_buffer).unwrap(),
                VulkanAllocatorCacheEntry {
                    block: res_block.clone(),
                },
            );
        }

        res
    }

    pub fn free_mem_raw(&mut self, mem: *mut c_void) {
        // try to find the buffer in the buffer cache first
        let res = self
            .mapped_memory_cache
            .remove(&std::ptr::NonNull::new(mem).unwrap());
        if let Some(mut entry) = res {
            // remove it here and exit the function
            if entry.block.is_cached {
                unsafe { &mut *entry.block.heap }.free(&entry.block.heap_data);
                self.staging_buffer_cache.can_shrink = true;
            } else {
                self.mem
                    .clean_buffer_pair(0, &mut entry.block.buffer, &mut entry.block.buffer_mem)
            }
        } else {
            let res = self
                .mapped_memory_cache_image
                .remove(&std::ptr::NonNull::new(mem).unwrap());
            if let Some(mut entry) = res {
                // remove it here and exit the function
                if entry.block.is_cached {
                    unsafe { &mut *entry.block.heap }.free(&entry.block.heap_data);
                    self.staging_buffer_cache_image.can_shrink = true;
                } else {
                    self.mem.clean_buffer_pair(
                        0,
                        &mut entry.block.buffer,
                        &mut entry.block.buffer_mem,
                    )
                }
            } else {
                panic!("memory that was tried to be deallocated was not found. That could mean it was already free'd (dobule free).");
            }
        }
    }

    pub fn free_mem(&mut self, mut mem: GraphicsBackendMemory) {
        self.free_mem_raw(mem.as_mut_slice().as_ptr() as *mut c_void);
    }

    fn queue_free_mem_frame(&mut self, mut mem: GraphicsBackendMemory, frame_index: u32) {
        let pointers_entry = self.cleanups.get_mut(&Some(frame_index));
        let pointers;
        match pointers_entry {
            Some(pointer_list) => pointers = pointer_list,
            None => {
                self.cleanups.insert(Some(frame_index), Vec::new());
                pointers = self.cleanups.get_mut(&Some(frame_index)).unwrap();
            }
        }
        pointers.push(std::ptr::NonNull::new(mem.as_mut_slice().as_ptr() as *mut c_void).unwrap());
    }

    pub fn upload_and_free_mem<F>(
        &mut self,
        mut mem: GraphicsBackendMemory,
        cur_image_index: u32,
        prepare_mem_range: F,
    ) where
        F: FnOnce(&SDeviceMemoryBlock, &SMemoryHeapQueueElement),
    {
        let mem_block;
        let heap_queue_el;
        let res = self
            .mapped_memory_cache
            .get(&std::ptr::NonNull::new(mem.as_mut_slice().as_ptr() as *mut c_void).unwrap());
        if let Some(entry) = res {
            mem_block = &entry.block.buffer_mem;
            heap_queue_el = &entry.block.heap_data;
        } else {
            let res = self
                .mapped_memory_cache_image
                .get(&std::ptr::NonNull::new(mem.as_mut_slice().as_ptr() as *mut c_void).unwrap());
            if let Some(entry) = res {
                mem_block = &entry.block.buffer_mem;
                heap_queue_el = &entry.block.heap_data;
            } else {
                panic!("memory was not allocated, maybe it was free'd already.");
            }
        }

        prepare_mem_range(mem_block, heap_queue_el);
        self.queue_free_mem_frame(mem, cur_image_index);
    }

    pub fn free_mems_of_frame(&mut self, frame_index: u32) {
        let pointers = self.cleanups.remove(&Some(frame_index));
        if let Some(mut pointer_list) = pointers {
            for pointer in pointer_list.drain(..) {
                self.free_mem_raw(pointer.as_ptr());
            }
        }
        self.staging_buffer_cache.shrink(&self.mem.device);
        self.staging_buffer_cache_image.shrink(&self.mem.device);
    }

    pub fn get_mem_block(
        &self,
        mem: *mut c_void,
    ) -> anyhow::Result<&SMemoryBlock<THREADED_STAGING_BUFFER_CACHE_ID>, ()> {
        let res = self
            .mapped_memory_cache
            .get(&std::ptr::NonNull::new(mem).unwrap());
        if let Some(entry) = res {
            Ok(&entry.block)
        } else {
            Err(())
        }
    }

    pub fn get_mem_block_image(
        &self,
        mem: *mut c_void,
    ) -> anyhow::Result<&SMemoryBlock<THREADED_STAGING_BUFFER_IMAGE_CACHE_ID>, ()> {
        let res = self
            .mapped_memory_cache_image
            .get(&std::ptr::NonNull::new(mem).unwrap());
        if let Some(entry) = res {
            Ok(&entry.block)
        } else {
            Err(())
        }
    }
}
