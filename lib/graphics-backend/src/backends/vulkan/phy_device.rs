use std::{
    ffi::CStr,
    sync::{Arc, RwLock},
};

use anyhow::anyhow;
use ash::vk;
use graphics_types::commands::ETWGraphicsGPUType;
use hiarc::Hiarc;
use log::{info, warn};

use super::{
    common::{GraphicsGpuItem, TTWGraphicsGPUList},
    instance::Instance,
    vulkan_config::Config,
    vulkan_dbg::is_verbose_mode,
    vulkan_limits::Limits,
    Options,
};

#[derive(Debug, Hiarc)]
pub struct PhyDevice {
    pub gpu_list: TTWGraphicsGPUList,
    pub limits: Limits,
    pub config: RwLock<Config>,
    pub renderer_name: String,
    pub vendor_name: String,
    pub version_name: String,
    #[hiarc_skip_unsafe]
    pub cur_device: vk::PhysicalDevice,
    #[hiarc_skip_unsafe]
    pub raw_device_props: vk::PhysicalDeviceProperties,
    pub queue_node_index: u32,

    // take an instance of the vk instance. it must outlive the device
    pub instance: Arc<Instance>,
}

impl PhyDevice {
    // from:
    // https://github.com/SaschaWillems/vulkan.gpuinfo.org/blob/5c3986798afc39d736b825bf8a5fbf92b8d9ed49/includes/functions.php#L364
    fn get_driver_verson(driver_version: u32, vendor_id: u32) -> String {
        // NVIDIA
        if vendor_id == 4318 {
            format!(
                "{}.{}.{}.{}",
                (driver_version >> 22) & 0x3ff,
                (driver_version >> 14) & 0x0ff,
                (driver_version >> 6) & 0x0ff,
                (driver_version) & 0x003f
            )
        }
        // windows only
        else if vendor_id == 0x8086 {
            format!("{}.{}", (driver_version >> 14), (driver_version) & 0x3fff)
        } else {
            // Use Vulkan version conventions if vendor mapping is not available
            format!(
                "{}.{}.{}",
                (driver_version >> 22),
                (driver_version >> 12) & 0x3ff,
                driver_version & 0xfff
            )
        }
    }

    fn vk_gputype_to_graphics_gputype(vk_gpu_type: vk::PhysicalDeviceType) -> ETWGraphicsGPUType {
        if vk_gpu_type == vk::PhysicalDeviceType::DISCRETE_GPU {
            return ETWGraphicsGPUType::Discrete;
        } else if vk_gpu_type == vk::PhysicalDeviceType::INTEGRATED_GPU {
            return ETWGraphicsGPUType::Integrated;
        } else if vk_gpu_type == vk::PhysicalDeviceType::VIRTUAL_GPU {
            return ETWGraphicsGPUType::Virtual;
        } else if vk_gpu_type == vk::PhysicalDeviceType::CPU {
            return ETWGraphicsGPUType::CPU;
        }

        ETWGraphicsGPUType::CPU
    }

    fn update_texture_capabilities(&self) {
        // check if image format supports linear blitting
        let format_properties = unsafe {
            self.instance
                .vk_instance
                .get_physical_device_format_properties(self.cur_device, vk::Format::R8G8B8A8_UNORM)
        };
        if !(format_properties.optimal_tiling_features
            & vk::FormatFeatureFlags::SAMPLED_IMAGE_FILTER_LINEAR)
            .is_empty()
        {
            self.config.write().unwrap().allows_linear_blitting = true;
        }
        if !(format_properties.optimal_tiling_features & vk::FormatFeatureFlags::BLIT_SRC)
            .is_empty()
            && !(format_properties.optimal_tiling_features & vk::FormatFeatureFlags::BLIT_DST)
                .is_empty()
        {
            self.config.write().unwrap().optimal_rgba_image_blitting = true;
        }
        // check if image format supports blitting to linear tiled images
        if !(format_properties.linear_tiling_features & vk::FormatFeatureFlags::BLIT_DST).is_empty()
        {
            self.config.write().unwrap().linear_rgba_image_blitting = true;
        }
    }

    pub fn update_surface_texture_capabilities(&self, surface_format: vk::Format) {
        let format_properties = unsafe {
            self.instance
                .vk_instance
                .get_physical_device_format_properties(self.cur_device, surface_format)
        };
        if !(format_properties.optimal_tiling_features & vk::FormatFeatureFlags::BLIT_SRC)
            .is_empty()
        {
            self.config
                .write()
                .unwrap()
                .optimal_swap_chain_image_blitting = true;
        }
    }

    pub fn new(
        instance: Arc<Instance>,
        options: &Options,
        is_headless: bool,
    ) -> anyhow::Result<Arc<Self>> {
        let device_list = unsafe { instance.vk_instance.enumerate_physical_devices() }?;

        let mut gpu_list = TTWGraphicsGPUList::default();

        let mut device_prop_list = Vec::<vk::PhysicalDeviceProperties>::new();
        device_prop_list.resize(device_list.len(), Default::default());
        gpu_list.gpus.reserve(device_list.len());

        let mut found_device_index: usize = 0;
        let mut found_gpu_type: usize = ETWGraphicsGPUType::Invalid as usize;

        let mut auto_gpu_type = ETWGraphicsGPUType::Invalid;

        let is_auto_gpu: bool = true; // TODO str_comp("auto" /* TODO: g_Config.m_GfxGPUName */, "auto") == 0;

        let vk_backend_major: usize = 1;
        let vk_backend_minor: usize = if is_headless { 2 } else { 1 };

        for (index, cur_device) in device_list.iter().enumerate() {
            device_prop_list[index] = unsafe {
                instance
                    .vk_instance
                    .get_physical_device_properties(*cur_device)
            };

            let device_prop = &mut device_prop_list[index];

            let gpu_type = Self::vk_gputype_to_graphics_gputype(device_prop.device_type);

            let dev_api_major: i32 = vk::api_version_major(device_prop.api_version) as i32;
            let dev_api_minor: i32 = vk::api_version_minor(device_prop.api_version) as i32;

            if dev_api_major > vk_backend_major as i32
                || (dev_api_major == vk_backend_major as i32
                    && dev_api_minor >= vk_backend_minor as i32)
            {
                let mut new_gpu = GraphicsGpuItem::default();
                new_gpu.name = unsafe {
                    CStr::from_ptr(device_prop.device_name.as_ptr())
                        .to_str()
                        .unwrap()
                        .to_string()
                };
                new_gpu.gpu_type = gpu_type as u32;
                gpu_list.gpus.push(new_gpu);

                if (gpu_type as usize) < auto_gpu_type as usize {
                    gpu_list.auto_gpu.name = unsafe {
                        CStr::from_ptr(device_prop.device_name.as_ptr())
                            .to_str()
                            .unwrap()
                            .to_string()
                    };
                    gpu_list.auto_gpu.gpu_type = gpu_type as u32;

                    auto_gpu_type = gpu_type;
                }

                if ((is_auto_gpu && (gpu_type as usize) < found_gpu_type)
                    || unsafe {
                        CStr::from_ptr(device_prop.device_name.as_ptr())
                            .to_str()
                            .unwrap()
                            == "auto" /* TODO: g_Config.m_GfxGPUName */
                    })
                    && (dev_api_major > vk_backend_major as i32
                        || (dev_api_major == vk_backend_major as i32
                            && dev_api_minor >= vk_backend_minor as i32))
                {
                    found_device_index = index;
                    found_gpu_type = gpu_type as usize;
                }
            }
        }

        if gpu_list.gpus.is_empty() {
            return Err(anyhow!("No devices with required vulkan version found."));
        }

        let device_prop = &device_prop_list[found_device_index];

        let dev_api_major: i32 = vk::api_version_major(device_prop.api_version) as i32;
        let dev_api_minor: i32 = vk::api_version_minor(device_prop.api_version) as i32;
        let dev_api_patch: i32 = vk::api_version_patch(device_prop.api_version) as i32;

        let renderer_name = unsafe {
            CStr::from_ptr(device_prop.device_name.as_ptr())
                .to_str()
                .unwrap()
                .to_string()
        };
        let vendor_name_str = match device_prop.vendor_id {
            0x1002 => "AMD",
            0x1010 => "ImgTec",
            0x106B => "Apple",
            0x10DE => "NVIDIA",
            0x13B5 => "ARM",
            0x5143 => "Qualcomm",
            0x8086 => "INTEL",
            0x10005 => "Mesa",
            _ => {
                warn!("unknown gpu vendor {}", device_prop.vendor_id);
                "unknown"
            }
        };

        let mut limits = Limits::default();
        let vendor_name = vendor_name_str.to_string();
        let version_name = format!(
            "Vulkan {}.{}.{} (driver: {})",
            dev_api_major,
            dev_api_minor,
            dev_api_patch,
            Self::get_driver_verson(device_prop.driver_version, device_prop.vendor_id)
        );

        info!("{}, {}", version_name, vendor_name);

        // get important device limits
        limits.non_coherent_mem_alignment = device_prop.limits.non_coherent_atom_size;
        limits.optimal_image_copy_mem_alignment =
            device_prop.limits.optimal_buffer_copy_offset_alignment;
        limits.max_texture_size = device_prop.limits.max_image_dimension2_d;
        limits.max_sampler_anisotropy = device_prop.limits.max_sampler_anisotropy as u32;

        limits.min_uniform_align = device_prop.limits.min_uniform_buffer_offset_alignment as u32;
        limits.max_multi_sample = device_prop.limits.framebuffer_color_sample_counts;

        if is_verbose_mode(options.dbg.gfx) {
            info!(
                "device prop: non-coherent align: {}\
                , optimal image copy align: {}\
                , max texture size: {}\
                , max sampler anisotropy: {}",
                limits.non_coherent_mem_alignment,
                limits.optimal_image_copy_mem_alignment,
                limits.max_texture_size,
                limits.max_sampler_anisotropy
            );
            info!(
                "device prop: min uniform align: {}, multi sample: {}",
                limits.min_uniform_align,
                limits.max_multi_sample.as_raw()
            );
        }

        let cur_device = device_list[found_device_index];

        let queue_prop_list = unsafe {
            instance
                .vk_instance
                .get_physical_device_queue_family_properties(cur_device)
        };
        if queue_prop_list.is_empty() {
            return Err(anyhow!("No vulkan queue family properties found."));
        }

        let mut queue_node_index: u32 = u32::MAX;
        for (i, queue_prop) in queue_prop_list.iter().enumerate() {
            if queue_prop.queue_count > 0
                && !(queue_prop.queue_flags & vk::QueueFlags::GRAPHICS).is_empty()
            {
                queue_node_index = i as u32;
            }
            /*if(vQueuePropList[i].queue_count > 0 && (vQueuePropList[i].queue_flags &
            vk::QueueFlags::COMPUTE))
            {
                QueueNodeIndex = i;
            }*/
        }

        if queue_node_index == u32::MAX {
            return Err(anyhow!(
                "No vulkan queue found that matches the requirements: graphics queue.",
            ));
        }

        let res = Self {
            instance,

            gpu_list,
            limits,
            config: RwLock::new(Config {
                allows_linear_blitting: Default::default(),
                optimal_swap_chain_image_blitting: Default::default(),
                optimal_rgba_image_blitting: Default::default(),
                linear_rgba_image_blitting: Default::default(),
            }),
            renderer_name,
            vendor_name,
            version_name,
            cur_device,
            raw_device_props: *device_prop,
            queue_node_index,
        };
        res.update_texture_capabilities();

        Ok(Arc::new(res))
    }
}
