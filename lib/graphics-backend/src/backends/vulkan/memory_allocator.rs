use std::sync::{
    atomic::{AtomicU64, AtomicU8},
    Arc,
};

use ash::vk;

use super::{
    common::verbose_deallocated_memory, vulkan_dbg::is_verbose, vulkan_types::EMemoryBlockUsage,
};

#[derive(Debug, Default)]
struct MemoryCleanupOfFrame {
    unmap_device_memory: Vec<vk::DeviceMemory>,
    device_memory: Vec<(vk::DeviceMemory, vk::DeviceSize, EMemoryBlockUsage)>,
    buffers: Vec<vk::Buffer>,
    images: Vec<vk::Image>,
    image_views: Vec<vk::ImageView>,
    descriptor_sets: Vec<(Vec<vk::DescriptorSet>, vk::DescriptorPool, Arc<AtomicU64>)>,
    descriptor_pools: Vec<vk::DescriptorPool>,
    descriptor_set_layouts: Vec<vk::DescriptorSetLayout>,

    fences: Vec<vk::Fence>,
    semaphores: Vec<vk::Semaphore>,
    command_buffers: Vec<(vk::CommandPool, Vec<vk::CommandBuffer>)>,
    command_pools: Vec<vk::CommandPool>,
}

/// Makes sure memory is always deallocated. additionally supports to delay memory
/// Note this is not meant as GPU VRAM allocator, but rather any device related memory
/// that also includes images etc.
pub struct MemoryAllocator {
    device: ash::Device,

    dbg: Arc<AtomicU8>,

    cur_frame_index: usize,

    frame_cleanups: Vec<MemoryCleanupOfFrame>,

    texture_memory_usage: Arc<AtomicU64>,
    buffer_memory_usage: Arc<AtomicU64>,
    stream_memory_usage: Arc<AtomicU64>,
    staging_memory_usage: Arc<AtomicU64>,
}

impl MemoryAllocator {
    pub fn new(
        device: ash::Device,

        dbg: Arc<AtomicU8>,

        texture_memory_usage: Arc<AtomicU64>,
        buffer_memory_usage: Arc<AtomicU64>,
        stream_memory_usage: Arc<AtomicU64>,
        staging_memory_usage: Arc<AtomicU64>,
    ) -> Self {
        Self {
            device,
            cur_frame_index: 0,

            dbg,

            frame_cleanups: Vec::new(),

            texture_memory_usage,
            buffer_memory_usage,
            stream_memory_usage,
            staging_memory_usage,
        }
    }

    fn clear_frames(&mut self) {
        // if the allocator is dropped, still clear all memory
        let len = self.frame_cleanups.len();
        for index in 0..len {
            self.clear_frame(index);
        }
    }

    pub fn set_frame_count(&mut self, frame_count: usize) {
        self.clear_frames();
        self.frame_cleanups
            .resize_with(frame_count, || Default::default());
    }

    pub fn set_frame_index(&mut self, frame_index: usize) {
        self.cur_frame_index = frame_index;
        self.clear_frame(frame_index);
    }

    pub fn free_device_memory(
        &mut self,
        memory: vk::DeviceMemory,
        size: vk::DeviceSize,
        usage_type: EMemoryBlockUsage,
    ) {
        self.frame_cleanups[self.cur_frame_index]
            .device_memory
            .push((memory, size, usage_type));
    }

    fn free_device_memory_impl(
        &self,
        memory: vk::DeviceMemory,
        size: vk::DeviceSize,
        usage_type: EMemoryBlockUsage,
    ) {
        match usage_type {
            EMemoryBlockUsage::Texture => {
                self.texture_memory_usage
                    .fetch_sub(size, std::sync::atomic::Ordering::Relaxed);
            }
            EMemoryBlockUsage::Buffer => {
                self.buffer_memory_usage
                    .fetch_sub(size, std::sync::atomic::Ordering::Relaxed);
            }
            EMemoryBlockUsage::Stream => {
                self.stream_memory_usage
                    .fetch_sub(size, std::sync::atomic::Ordering::Relaxed);
            }
            EMemoryBlockUsage::Staging => {
                self.staging_memory_usage
                    .fetch_sub(size, std::sync::atomic::Ordering::Relaxed);
            }
            EMemoryBlockUsage::Dummy => {}
        };

        if is_verbose(&self.dbg) {
            verbose_deallocated_memory(size, usage_type);
        }

        unsafe {
            self.device.free_memory(memory, None);
        }
    }

    pub fn unmap_device_memory(&mut self, memory: vk::DeviceMemory) {
        self.frame_cleanups[self.cur_frame_index]
            .unmap_device_memory
            .push(memory);
    }

    fn unmap_device_memory_impl(&self, memory: vk::DeviceMemory) {
        unsafe {
            self.device.unmap_memory(memory);
        }
    }

    pub fn free_buffer(&mut self, buffer: vk::Buffer) {
        self.frame_cleanups[self.cur_frame_index]
            .buffers
            .push(buffer);
    }

    fn free_buffer_impl(&self, buffer: vk::Buffer) {
        unsafe { self.device.destroy_buffer(buffer, None) };
    }

    pub fn free_image(&mut self, img: vk::Image) {
        self.frame_cleanups[self.cur_frame_index].images.push(img);
    }

    fn free_image_impl(&self, img: vk::Image) {
        unsafe { self.device.destroy_image(img, None) };
    }

    pub fn free_image_view(&mut self, img_view: vk::ImageView) {
        self.frame_cleanups[self.cur_frame_index]
            .image_views
            .push(img_view);
    }

    fn free_image_view_impl(&self, img_view: vk::ImageView) {
        unsafe { self.device.destroy_image_view(img_view, None) };
    }

    pub fn free_descriptor_sets(
        &mut self,
        sets: Vec<vk::DescriptorSet>,
        pool: vk::DescriptorPool,
        pool_cur_size: Arc<AtomicU64>,
    ) {
        self.frame_cleanups[self.cur_frame_index]
            .descriptor_sets
            .push((sets, pool, pool_cur_size));
    }

    fn free_descriptor_set_impl(
        &self,
        sets: Vec<vk::DescriptorSet>,
        pool: vk::DescriptorPool,
        pool_cur_size: Arc<AtomicU64>,
    ) {
        unsafe {
            self.device.free_descriptor_sets(pool, &sets).unwrap();
        }
        pool_cur_size.fetch_sub(sets.len() as u64, std::sync::atomic::Ordering::SeqCst);
    }

    pub fn free_descriptor_pool(&mut self, pool: vk::DescriptorPool) {
        self.frame_cleanups[self.cur_frame_index]
            .descriptor_pools
            .push(pool);
    }

    fn free_descriptor_pool_impl(&self, pool: vk::DescriptorPool) {
        unsafe {
            self.device.destroy_descriptor_pool(pool, None);
        }
    }

    pub fn free_descriptor_set_layout(&mut self, layout: vk::DescriptorSetLayout) {
        self.frame_cleanups[self.cur_frame_index]
            .descriptor_set_layouts
            .push(layout);
    }

    fn free_descriptor_set_layout_impl(&self, layout: vk::DescriptorSetLayout) {
        unsafe {
            self.device.destroy_descriptor_set_layout(layout, None);
        }
    }

    pub fn free_fence(&mut self, fence: vk::Fence) {
        self.frame_cleanups[self.cur_frame_index].fences.push(fence);
    }

    fn free_fence_impl(&self, fence: vk::Fence) {
        unsafe {
            self.device.destroy_fence(fence, None);
        }
    }

    pub fn free_semaphore(&mut self, semaphore: vk::Semaphore) {
        self.frame_cleanups[self.cur_frame_index]
            .semaphores
            .push(semaphore);
    }

    fn free_semaphore_impl(&self, semaphore: vk::Semaphore) {
        unsafe {
            self.device.destroy_semaphore(semaphore, None);
        }
    }

    pub fn free_command_buffers(
        &mut self,
        command_pool: vk::CommandPool,
        command_buffers: Vec<vk::CommandBuffer>,
    ) {
        self.frame_cleanups[self.cur_frame_index]
            .command_buffers
            .push((command_pool, command_buffers));
    }

    fn free_command_buffers_impl(
        &self,
        command_pool: vk::CommandPool,
        command_buffer: Vec<vk::CommandBuffer>,
    ) {
        unsafe {
            self.device
                .free_command_buffers(command_pool, &command_buffer);
        }
    }

    pub fn free_command_pool(&mut self, command_pool: vk::CommandPool) {
        self.frame_cleanups[self.cur_frame_index]
            .command_pools
            .push(command_pool);
    }

    fn free_command_pool_impl(&self, command_pool: vk::CommandPool) {
        unsafe {
            self.device.destroy_command_pool(command_pool, None);
        }
    }

    fn clear_frame(&mut self, frame_index: usize) {
        let frame = &mut self.frame_cleanups[frame_index];
        let mut unmap_memories = std::mem::take(&mut frame.unmap_device_memory);
        let mut memories = std::mem::take(&mut frame.device_memory);
        let mut buffers = std::mem::take(&mut frame.buffers);
        let mut images = std::mem::take(&mut frame.images);
        let mut image_views = std::mem::take(&mut frame.image_views);
        let mut descriptor_sets = std::mem::take(&mut frame.descriptor_sets);
        let mut descriptor_pools = std::mem::take(&mut frame.descriptor_pools);
        let mut descriptor_set_layouts = std::mem::take(&mut frame.descriptor_set_layouts);

        let mut fences = std::mem::take(&mut frame.fences);
        let mut semaphores = std::mem::take(&mut frame.semaphores);
        let mut command_buffers = std::mem::take(&mut frame.command_buffers);
        let mut command_pools = std::mem::take(&mut frame.command_pools);

        // IMPORTANT: the free operations have to be in a specific hirarchical order:
        // device memory must be last. A descriptor pool must be cleared after descriptor sets etc.

        // descriptors are used for both images & buffers (uniform)
        for (set, pool, pool_cur_size) in descriptor_sets.drain(..) {
            self.free_descriptor_set_impl(set, pool, pool_cur_size);
        }

        // image order: views, images
        for img_view in image_views.drain(..) {
            self.free_image_view_impl(img_view);
        }
        for img in images.drain(..) {
            self.free_image_impl(img);
        }

        // buffers should be independent of images.. but still keep it behind them
        for buffer in buffers.drain(..) {
            self.free_buffer_impl(buffer);
        }

        // descriptor pools & layouts however, should be cleared late, after buffers & textures
        for pool in descriptor_pools.drain(..) {
            self.free_descriptor_pool_impl(pool);
        }
        for layout in descriptor_set_layouts.drain(..) {
            self.free_descriptor_set_layout_impl(layout);
        }

        // fences & semaphores
        for fence in fences.drain(..) {
            self.free_fence_impl(fence);
        }
        for semaphore in semaphores.drain(..) {
            self.free_semaphore_impl(semaphore);
        }

        // command buffers & pool
        for (command_pool, command_buffer) in command_buffers.drain(..) {
            self.free_command_buffers_impl(command_pool, command_buffer);
        }
        for command_pool in command_pools.drain(..) {
            self.free_command_pool_impl(command_pool);
        }

        // unmapping memory
        for memory in unmap_memories.drain(..) {
            self.unmap_device_memory_impl(memory);
        }

        // always last: device memory
        for (memory, size, usage_type) in memories.drain(..) {
            self.free_device_memory_impl(memory, size, usage_type);
        }
    }
}

impl Drop for MemoryAllocator {
    fn drop(&mut self) {
        self.clear_frames()
    }
}
