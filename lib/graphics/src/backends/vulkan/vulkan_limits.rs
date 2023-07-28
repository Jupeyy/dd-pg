use ash::vk;

#[derive(Default, Clone)]
pub struct Limits {
    pub non_coherent_mem_alignment: vk::DeviceSize,
    pub optimal_image_copy_mem_alignment: vk::DeviceSize,

    pub max_texture_size: u32,
    pub max_sampler_anisotropy: u32,
    pub max_multi_sample: vk::SampleCountFlags,

    pub min_uniform_align: u32,
}
