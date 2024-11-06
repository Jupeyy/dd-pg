use std::sync::Arc;

use ash::vk;
use hiarc::Hiarc;

use super::{frame_resources::FrameResources, logical_device::LogicalDevice};

#[derive(Debug, Hiarc)]
pub struct Sampler {
    #[hiarc_skip_unsafe]
    sampler: vk::Sampler,
    device: Arc<LogicalDevice>,
}

impl Sampler {
    pub fn new(
        device: &Arc<LogicalDevice>,
        max_sampler_anisotropy: u32,
        global_texture_lod_bias: f64,
        addr_mode_u: vk::SamplerAddressMode,
        addr_mode_v: vk::SamplerAddressMode,
        addr_mode_w: vk::SamplerAddressMode,
    ) -> anyhow::Result<Arc<Self>> {
        let mut sampler_info = vk::SamplerCreateInfo::default();
        sampler_info.mag_filter = vk::Filter::LINEAR;
        sampler_info.min_filter = vk::Filter::LINEAR;
        sampler_info.address_mode_u = addr_mode_u;
        sampler_info.address_mode_v = addr_mode_v;
        sampler_info.address_mode_w = addr_mode_w;
        sampler_info.anisotropy_enable = vk::FALSE;
        sampler_info.max_anisotropy = max_sampler_anisotropy as f32;
        sampler_info.border_color = vk::BorderColor::INT_OPAQUE_BLACK;
        sampler_info.unnormalized_coordinates = vk::FALSE;
        sampler_info.compare_enable = vk::FALSE;
        sampler_info.compare_op = vk::CompareOp::ALWAYS;
        sampler_info.mipmap_mode = vk::SamplerMipmapMode::LINEAR;
        sampler_info.mip_lod_bias = global_texture_lod_bias as f32;
        sampler_info.min_lod = -1000.0;
        sampler_info.max_lod = 1000.0;

        let sampler = unsafe { device.device.create_sampler(&sampler_info, None) }?;

        Ok(Arc::new(Self {
            device: device.clone(),
            sampler,
        }))
    }

    pub fn sampler(self: &Arc<Self>, frame_resouces: &mut FrameResources) -> vk::Sampler {
        frame_resouces.samplers.push(self.clone());

        self.sampler
    }
}

impl Drop for Sampler {
    fn drop(&mut self) {
        unsafe {
            self.device.device.destroy_sampler(self.sampler, None);
        }
    }
}
