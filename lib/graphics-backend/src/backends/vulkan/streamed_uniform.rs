use std::sync::Arc;

use anyhow::anyhow;
use ash::vk;
use libc::c_void;

use super::{
    buffer::Buffer,
    descriptor_layout::DescriptorSetLayout,
    descriptor_set::DescriptorSet,
    logical_device::LogicalDevice,
    vulkan_allocator::VulkanAllocator,
    vulkan_mem::Memory,
    vulkan_types::{SDeviceDescriptorPools, SFrameUniformBuffers, SStreamMemory},
};

#[derive(Debug)]
pub struct StreamedUniform {
    uniform_buffer_descr_pools: Vec<SDeviceDescriptorPools>,

    sprite_multi_uniform_descriptor_set_layout: Arc<DescriptorSetLayout>,
    quad_uniform_descriptor_set_layout: Arc<DescriptorSetLayout>,

    pub buffers: Vec<SStreamMemory<SFrameUniformBuffers>>,

    device: Arc<LogicalDevice>,
    mem: Memory,
}

impl StreamedUniform {
    fn create_uniform_descr_pools(
        device: &Arc<LogicalDevice>,
        thread_count: usize,
    ) -> anyhow::Result<Vec<SDeviceDescriptorPools>> {
        let mut uniform_buffer_descr_pools: Vec<SDeviceDescriptorPools> = Vec::new();

        uniform_buffer_descr_pools.resize(thread_count, Default::default());

        for uniform_buffer_descr_pool in &mut uniform_buffer_descr_pools {
            uniform_buffer_descr_pool.is_uniform_pool = true;
            uniform_buffer_descr_pool.default_alloc_size = 512;
        }

        for uniform_buffer_descr_pool in &mut uniform_buffer_descr_pools {
            VulkanAllocator::allocate_descriptor_pool(device, uniform_buffer_descr_pool, 64)?;
        }

        Ok(uniform_buffer_descr_pools)
    }

    pub fn new(
        device: Arc<LogicalDevice>,
        mem: Memory,

        thread_count: usize,

        sprite_multi_uniform_descriptor_set_layout: Arc<DescriptorSetLayout>,
        quad_uniform_descriptor_set_layout: Arc<DescriptorSetLayout>,
    ) -> anyhow::Result<Arc<spin::Mutex<Self>>> {
        let uniform_buffer_descr_pools = Self::create_uniform_descr_pools(&device, thread_count)?;

        let streamed_uniform_buffers = Default::default();

        Ok(Arc::new(spin::Mutex::new(Self {
            uniform_buffer_descr_pools,
            buffers: streamed_uniform_buffers,
            sprite_multi_uniform_descriptor_set_layout,
            quad_uniform_descriptor_set_layout,
            device,
            mem,
        })))
    }

    pub fn get_uniform_buffer_object_impl<
        TName,
        const INSTANCE_MAX_PARTICLE_COUNT: usize,
        const MAX_INSTANCES: usize,
    >(
        &mut self,
        render_thread_index: usize,
        requires_shared_stages_descriptor: bool,
        ptr_raw_data: *const c_void,
        data_size: usize,
        cur_image_index: u32,
    ) -> anyhow::Result<Arc<DescriptorSet>> {
        let mem = &self.mem;
        let device = &self.device;
        let pools = &mut self.uniform_buffer_descr_pools[render_thread_index];
        let sprite_descr_layout = &self.sprite_multi_uniform_descriptor_set_layout;
        let quad_descr_layout = &self.quad_uniform_descriptor_set_layout;
        let stream_uniform_buffer = &mut self.buffers[render_thread_index];
        let mut new_mem_func = move |mem: &mut SFrameUniformBuffers,
                                     buffer: &Arc<Buffer>,
                                     mem_offset: vk::DeviceSize|
              -> bool {
            match VulkanAllocator::create_uniform_descriptor_sets(
                &device,
                pools,
                sprite_descr_layout,
                1,
                buffer,
                INSTANCE_MAX_PARTICLE_COUNT * std::mem::size_of::<TName>(),
                mem_offset,
            ) {
                Ok(mut descriptors) => {
                    mem.uniform_sets[0] = Some(descriptors.remove(0));
                }
                Err(_) => {
                    return false;
                }
            }
            match VulkanAllocator::create_uniform_descriptor_sets(
                &device,
                pools,
                quad_descr_layout,
                1,
                buffer,
                INSTANCE_MAX_PARTICLE_COUNT * std::mem::size_of::<TName>(),
                mem_offset,
            ) {
                Ok(mut descriptors) => mem.uniform_sets[1] = Some(descriptors.remove(0)),
                Err(_) => {
                    return false;
                }
            }

            true
        };
        let res = VulkanAllocator::create_stream_buffer::<
            SFrameUniformBuffers,
            TName,
            INSTANCE_MAX_PARTICLE_COUNT,
            MAX_INSTANCES,
            true,
        >(
            mem,
            device,
            &mut new_mem_func,
            stream_uniform_buffer,
            vk::BufferUsageFlags::UNIFORM_BUFFER,
            ptr_raw_data,
            data_size,
            cur_image_index,
        );
        if res.is_err() {
            return Err(anyhow!("Could not create stream buffer"));
        }

        let (ptr_mem, _, _, _) = res.unwrap();

        Ok(
            unsafe { &mut *ptr_mem }.uniform_sets[if requires_shared_stages_descriptor {
                1
            } else {
                0
            }]
            .as_ref()
            .unwrap()
            .clone(),
        )
    }
}
