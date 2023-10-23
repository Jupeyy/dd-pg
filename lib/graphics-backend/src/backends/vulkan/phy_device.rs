use std::{
    ffi::CStr,
    sync::{Arc, RwLock},
};

use anyhow::anyhow;
use ash::vk;
use base_log::log::{LogLevel, SystemLogGroup, SystemLogInterface};
use graphics_types::command_buffer::ETWGraphicsGPUType;

use super::{
    common::{STWGraphicGPUItem, TTWGraphicsGPUList},
    instance::Instance,
    vulkan_config::Config,
    vulkan_dbg::is_verbose_mode,
    vulkan_limits::Limits,
    Options,
};

#[derive(Debug)]
pub struct PhyDevice {
    pub gpu_list: TTWGraphicsGPUList,
    pub limits: Limits,
    pub config: RwLock<Config>,
    pub renderer_name: String,
    pub vendor_name: String,
    pub version_name: String,
    pub cur_device: vk::PhysicalDevice,
    pub queue_node_index: u32,

    // take an instance of the vk instance. it must outlive the device
    _instance: Arc<Instance>,
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

    pub fn new(
        instance: Arc<Instance>,
        options: &Options,
        logger: &SystemLogGroup,
        is_headless: bool,
    ) -> anyhow::Result<Arc<Self>> {
        let device_list = unsafe { instance.vk_instance.enumerate_physical_devices() }?;

        let renderer_name;
        let vendor_name;
        let version_name;
        let mut gpu_list = TTWGraphicsGPUList::default();

        let mut index: usize = 0;
        let mut device_prop_list = Vec::<vk::PhysicalDeviceProperties>::new();
        device_prop_list.resize(device_list.len(), Default::default());
        gpu_list.gpus.reserve(device_list.len());

        let mut found_device_index: usize = 0;
        let mut found_gpu_type: usize = ETWGraphicsGPUType::Invalid as usize;

        let mut auto_gpu_type = ETWGraphicsGPUType::Invalid;

        let is_auto_gpu: bool = true; // TODO str_comp("auto" /* TODO: g_Config.m_GfxGPUName */, "auto") == 0;

        let vk_backend_major: usize = 1;
        let vk_backend_minor: usize = if is_headless { 2 } else { 1 };

        for cur_device in &device_list {
            device_prop_list[index] = unsafe {
                instance
                    .vk_instance
                    .get_physical_device_properties(*cur_device)
            };

            let device_prop = &mut device_prop_list[index];

            let gpu_type = Self::vk_gputype_to_graphics_gputype(device_prop.device_type);

            let mut new_gpu = STWGraphicGPUItem::default();
            new_gpu.name = unsafe {
                CStr::from_ptr(device_prop.device_name.as_ptr())
                    .to_str()
                    .unwrap()
                    .to_string()
            };
            new_gpu.gpu_type = gpu_type as u32;
            gpu_list.gpus.push(new_gpu);

            index += 1;

            let dev_api_major: i32 = vk::api_version_major(device_prop.api_version) as i32;
            let dev_api_minor: i32 = vk::api_version_minor(device_prop.api_version) as i32;

            if (gpu_type as usize) < auto_gpu_type as usize
                && (dev_api_major > vk_backend_major as i32
                    || (dev_api_major == vk_backend_major as i32
                        && dev_api_minor >= vk_backend_minor as i32))
            {
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
                        .to_string()
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

        if found_device_index == 0 {
            found_device_index = 1;
        }

        let device_prop = &mut device_prop_list[found_device_index - 1];

        let dev_api_major: i32 = vk::api_version_major(device_prop.api_version) as i32;
        let dev_api_minor: i32 = vk::api_version_minor(device_prop.api_version) as i32;
        let dev_api_patch: i32 = vk::api_version_patch(device_prop.api_version) as i32;

        renderer_name = unsafe {
            CStr::from_ptr(device_prop.device_name.as_ptr())
                .to_str()
                .unwrap()
                .to_string()
        };
        let vendor_name_str: &str;
        match device_prop.vendor_id {
            0x1002 => vendor_name_str = "AMD",
            0x1010 => vendor_name_str = "ImgTec",
            0x106B => vendor_name_str = "Apple",
            0x10DE => vendor_name_str = "NVIDIA",
            0x13B5 => vendor_name_str = "ARM",
            0x5143 => vendor_name_str = "Qualcomm",
            0x8086 => vendor_name_str = "INTEL",
            0x10005 => vendor_name_str = "Mesa",
            _ => {
                logger
                    .log(LogLevel::Info)
                    .msg("unknown gpu vendor ")
                    .msg_var(&device_prop.vendor_id);
                vendor_name_str = "unknown"
            }
        }

        let mut limits = Limits::default();
        vendor_name = vendor_name_str.to_string();
        version_name = format!(
            "Vulkan {}.{}.{} (driver: {})",
            dev_api_major,
            dev_api_minor,
            dev_api_patch,
            Self::get_driver_verson(device_prop.driver_version, device_prop.vendor_id)
        );

        // get important device limits
        limits.non_coherent_mem_alignment = device_prop.limits.non_coherent_atom_size;
        limits.optimal_image_copy_mem_alignment =
            device_prop.limits.optimal_buffer_copy_offset_alignment;
        limits.max_texture_size = device_prop.limits.max_image_dimension2_d;
        limits.max_sampler_anisotropy = device_prop.limits.max_sampler_anisotropy as u32;

        limits.min_uniform_align = device_prop.limits.min_uniform_buffer_offset_alignment as u32;
        limits.max_multi_sample = device_prop.limits.framebuffer_color_sample_counts;

        if is_verbose_mode(options.dbg.gfx) {
            logger
                .log(LogLevel::Debug)
                .msg("device prop: non-coherent align: ")
                .msg_var(&limits.non_coherent_mem_alignment)
                .msg(", optimal image copy align: ")
                .msg_var(&limits.optimal_image_copy_mem_alignment)
                .msg(", max texture size: ")
                .msg_var(&limits.max_texture_size)
                .msg(", max sampler anisotropy: ")
                .msg_var(&limits.max_sampler_anisotropy);
            logger
                .log(LogLevel::Debug)
                .msg("device prop: min uniform align: ")
                .msg_var(&limits.min_uniform_align)
                .msg(", multi sample: ")
                .msg_var(&(limits.max_multi_sample.as_raw()));
        }

        let cur_device = device_list[found_device_index - 1];

        let queue_prop_list = unsafe {
            instance
                .vk_instance
                .get_physical_device_queue_family_properties(cur_device)
        };
        if queue_prop_list.len() == 0 {
            return Err(anyhow!("No vulkan queue family properties found."));
        }

        let mut queue_node_index: u32 = u32::MAX;
        for i in 0..queue_prop_list.len() {
            if queue_prop_list[i].queue_count > 0
                && !(queue_prop_list[i].queue_flags & vk::QueueFlags::GRAPHICS).is_empty()
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

        let multi_sampling_count = options.gl.fsaa_samples & 0xFFFFFFFE; // ignore the uneven bit, only even multi sampling works

        Ok(Arc::new(Self {
            _instance: instance,

            gpu_list,
            limits,
            config: RwLock::new(Config {
                multi_sampling_count,
                allows_linear_blitting: Default::default(),
                optimal_swap_chain_image_blitting: Default::default(),
                optimal_rgba_image_blitting: Default::default(),
                linear_rgba_image_blitting: Default::default(),
            }),
            renderer_name,
            vendor_name,
            version_name,
            cur_device,
            queue_node_index,
        }))
    }
}
