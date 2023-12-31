use std::sync::Arc;

use ash::vk;
use hiarc::HiArc;
use hiarc_macro::Hiarc;
use spin::RwLock;

use super::{
    buffer::Buffer, descriptor_layout::DescriptorSetLayout, descriptor_pool::DescriptorPool,
    frame_resources::FrameResources, image_view::ImageView, sampler::Sampler,
};

#[derive(Debug, Hiarc)]
pub struct DescriptorSets {
    sets: Vec<vk::DescriptorSet>,

    assigned_buffer: RwLock<Option<HiArc<Buffer>>>,
    assigned_img_view: RwLock<Option<HiArc<ImageView>>>,
    assigned_sampler: RwLock<Option<HiArc<Sampler>>>,

    _layout: HiArc<DescriptorSetLayout>,
    pub pool: HiArc<DescriptorPool>,
}

impl DescriptorSets {
    pub fn new(
        pool: HiArc<DescriptorPool>,

        set_count: usize,
        layout: &HiArc<DescriptorSetLayout>,
    ) -> anyhow::Result<HiArc<Self>> {
        let mut create_info = vk::DescriptorSetAllocateInfo::default();

        let layouts = vec![layout.layout; set_count];
        create_info.descriptor_set_count = set_count as u32;
        create_info.p_set_layouts = layouts.as_ptr();

        let pool_g = pool.pool.lock();
        create_info.descriptor_pool = *pool_g;
        let sets = unsafe { pool.device.device.allocate_descriptor_sets(&create_info) }?;
        drop(pool_g);

        pool.cur_size
            .fetch_add(sets.len() as u64, std::sync::atomic::Ordering::SeqCst);

        Ok(HiArc::new(Self {
            sets,
            pool,
            _layout: layout.clone(),
            assigned_buffer: Default::default(),
            assigned_img_view: Default::default(),
            assigned_sampler: Default::default(),
        }))
    }

    pub fn set(self: &Arc<Self>, frame_resources: &mut FrameResources) -> vk::DescriptorSet {
        frame_resources.descriptor_sets.push(self.clone());

        self.sets[0]
    }

    pub fn assign_uniform_buffer_to_sets(
        &self,
        buffer: &HiArc<Buffer>,
        offset: vk::DeviceSize,
        range_per_set: vk::DeviceSize,
    ) -> usize {
        let mut buffer_infos: Vec<vk::DescriptorBufferInfo> = Vec::with_capacity(self.sets.len());
        let mut descriptor_writes: Vec<vk::WriteDescriptorSet> =
            Vec::with_capacity(self.sets.len());
        let raw_buffer = buffer
            .inner_arc()
            .get_buffer(&mut FrameResources::new(None));
        for i in 0..self.sets.len() {
            buffer_infos.push(
                vk::DescriptorBufferInfo::builder()
                    .buffer(raw_buffer)
                    .offset(offset + (range_per_set * i as vk::DeviceSize))
                    .range(range_per_set)
                    .build(),
            );
        }

        for i in 0..self.sets.len() {
            descriptor_writes.push(
                vk::WriteDescriptorSet::builder()
                    .dst_set(self.sets[i])
                    .dst_binding(0)
                    .dst_array_element(0)
                    .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
                    .buffer_info(&buffer_infos[i..i + 1])
                    .build(),
            );
        }

        unsafe {
            self.pool
                .device
                .device
                .update_descriptor_sets(&descriptor_writes, &[]);
        }

        *self.assigned_buffer.write() = Some(buffer.clone());

        self.sets.len()
    }

    pub fn assign_texture_and_sampler_combined(
        &self,
        image_view: &HiArc<ImageView>,
        sampler: &HiArc<Sampler>,
    ) {
        let mut image_info = vk::DescriptorImageInfo::default();
        image_info.image_layout = vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL;
        image_info.image_view = image_view.inner_arc().view(&mut FrameResources::new(None));
        image_info.sampler = sampler.inner_arc().sampler(&mut FrameResources::new(None));

        let mut descriptor_writes: [vk::WriteDescriptorSet; 1] = Default::default();
        descriptor_writes[0].dst_set = self.sets[0];
        descriptor_writes[0].dst_binding = 0;
        descriptor_writes[0].dst_array_element = 0;
        descriptor_writes[0].descriptor_type = vk::DescriptorType::COMBINED_IMAGE_SAMPLER;
        descriptor_writes[0].descriptor_count = 1;
        descriptor_writes[0].p_image_info = &image_info;

        unsafe {
            self.pool
                .device
                .device
                .update_descriptor_sets(descriptor_writes.as_slice(), &[]);
        }

        *self.assigned_img_view.write() = Some(image_view.clone());
        *self.assigned_sampler.write() = Some(sampler.clone());
    }

    pub fn assign_texture(&self, image_view: &HiArc<ImageView>) {
        let mut image_info = vk::DescriptorImageInfo::default();
        image_info.image_layout = vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL;
        image_info.image_view = image_view.inner_arc().view(&mut FrameResources::new(None));

        let mut descriptor_writes: [vk::WriteDescriptorSet; 1] = Default::default();
        descriptor_writes[0].dst_set = self.sets[0];
        descriptor_writes[0].dst_binding = 0;
        descriptor_writes[0].dst_array_element = 0;
        descriptor_writes[0].descriptor_type = vk::DescriptorType::SAMPLED_IMAGE;
        descriptor_writes[0].descriptor_count = 1;
        descriptor_writes[0].p_image_info = &image_info;

        unsafe {
            self.pool
                .device
                .device
                .update_descriptor_sets(descriptor_writes.as_slice(), &[]);
        }

        *self.assigned_img_view.write() = Some(image_view.clone());
    }

    pub fn assign_sampler(&self, sampler: &HiArc<Sampler>) {
        let mut image_info = vk::DescriptorImageInfo::default();
        image_info.image_layout = vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL;
        image_info.sampler = sampler.inner_arc().sampler(&mut FrameResources::new(None));

        let mut descriptor_writes: [vk::WriteDescriptorSet; 1] = Default::default();
        descriptor_writes[0].dst_set = self.sets[0];
        descriptor_writes[0].dst_binding = 0;
        descriptor_writes[0].dst_array_element = 0;
        descriptor_writes[0].descriptor_type = vk::DescriptorType::SAMPLER;
        descriptor_writes[0].descriptor_count = 1;
        descriptor_writes[0].p_image_info = &image_info;

        unsafe {
            self.pool
                .device
                .device
                .update_descriptor_sets(descriptor_writes.as_slice(), &[]);
        }

        *self.assigned_sampler.write() = Some(sampler.clone());
    }
}

impl Drop for DescriptorSets {
    fn drop(&mut self) {
        let pool = *self.pool.pool.lock();

        unsafe {
            self.pool
                .device
                .device
                .free_descriptor_sets(pool, &self.sets)
                .unwrap();
        }
        self.pool
            .cur_size
            .fetch_sub(self.sets.len() as u64, std::sync::atomic::Ordering::SeqCst);
    }
}

#[derive(Debug, Hiarc)]
pub struct DescriptorSet {
    sets: HiArc<DescriptorSets>,
    set_index: usize,
}

impl DescriptorSet {
    pub fn set(&self, frame_resources: &mut FrameResources) -> vk::DescriptorSet {
        frame_resources
            .descriptor_sets
            .push(self.sets.inner_arc().clone());

        self.sets.sets[self.set_index]
    }
}

pub fn split_descriptor_sets(sets: &HiArc<DescriptorSets>) -> Vec<HiArc<DescriptorSet>> {
    sets.sets
        .iter()
        .enumerate()
        .map(|(index, _)| {
            HiArc::new(DescriptorSet {
                sets: sets.clone(),
                set_index: index,
            })
        })
        .collect()
}
