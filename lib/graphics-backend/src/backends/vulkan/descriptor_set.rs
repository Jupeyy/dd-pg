use std::sync::Arc;

use ash::vk;

use super::descriptor_pool::DescriptorPool;

#[derive(Debug)]
pub struct DescriptorSet {
    set: Vec<vk::DescriptorSet>,

    pub pool: Arc<DescriptorPool>,
}

impl DescriptorSet {
    pub fn new(
        pool: Arc<DescriptorPool>,
        mut create_info: vk::DescriptorSetAllocateInfo,
    ) -> anyhow::Result<Arc<Self>> {
        let pool_g = pool.pool.lock();
        create_info.descriptor_pool = *pool_g;
        let set = unsafe { pool.device.device.allocate_descriptor_sets(&create_info) }?;
        drop(pool_g);

        pool.cur_size
            .fetch_add(set.len() as u64, std::sync::atomic::Ordering::SeqCst);

        Ok(Arc::new(Self { set, pool }))
    }

    pub fn set(&self) -> vk::DescriptorSet {
        self.set[0]
    }
}

impl Drop for DescriptorSet {
    fn drop(&mut self) {
        let pool = *self.pool.pool.lock();
        self.pool
            .device
            .memory_allocator
            .lock()
            .free_descriptor_sets(
                std::mem::take(&mut self.set),
                pool,
                self.pool.cur_size.clone(),
            );
    }
}
