use std::{
    ffi::{CStr, CString},
    ops::Deref,
    sync::{atomic::AtomicU64, Arc},
};

use ash::vk;
use config::config::AtomicEDebugGFXModes;
use hiarc::HiArc;
use hiarc_macro::Hiarc;

use super::phy_device::PhyDevice;

pub struct DeviceWrapper(ash::Device);

impl Deref for DeviceWrapper {
    type Target = ash::Device;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Drop for DeviceWrapper {
    fn drop(&mut self) {
        unsafe {
            self.0.destroy_device(None);
        }
    }
}

#[derive(Hiarc)]
pub struct LogicalDevice {
    pub texture_memory_usage: Arc<AtomicU64>,
    pub buffer_memory_usage: Arc<AtomicU64>,
    pub stream_memory_usage: Arc<AtomicU64>,
    pub staging_memory_usage: Arc<AtomicU64>,

    pub is_headless: bool,

    pub dbg: Arc<AtomicEDebugGFXModes>,
    // has to outlive the memory allocator
    pub device: DeviceWrapper,
    // has to outlive the logical device
    pub phy_device: HiArc<PhyDevice>,
}

impl std::fmt::Debug for LogicalDevice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LogicalDevice")
            .field("phy_device", &self.phy_device)
            .finish()
    }
}

impl LogicalDevice {
    pub fn new(
        phy_gpu: HiArc<PhyDevice>,
        graphics_queue_index: u32,
        instance: &ash::Instance,
        layers: &Vec<String>,
        is_headless: bool,

        dbg: Arc<AtomicEDebugGFXModes>,

        texture_memory_usage: Arc<AtomicU64>,
        buffer_memory_usage: Arc<AtomicU64>,
        stream_memory_usage: Arc<AtomicU64>,
        staging_memory_usage: Arc<AtomicU64>,
    ) -> anyhow::Result<HiArc<Self>> {
        let device = Self::create_logical_device(
            &phy_gpu,
            graphics_queue_index,
            instance,
            layers,
            is_headless,
        )?;

        Ok(HiArc::new(Self {
            device: DeviceWrapper(device.clone()),
            phy_device: phy_gpu,

            is_headless,

            texture_memory_usage,
            buffer_memory_usage,
            stream_memory_usage,
            staging_memory_usage,

            dbg,
        }))
    }

    fn create_logical_device(
        phy_gpu: &HiArc<PhyDevice>,
        graphics_queue_index: u32,
        instance: &ash::Instance,
        layers: &Vec<String>,
        is_headless: bool,
    ) -> anyhow::Result<ash::Device> {
        let mut layer_cnames = Vec::<*const libc::c_char>::new();
        let mut layer_cnames_helper = Vec::<CString>::new();
        layer_cnames.reserve(layers.len());
        layer_cnames_helper.reserve(layers.len());
        for layer in layers {
            let mut bytes = layer.clone().into_bytes();
            bytes.push(0);
            layer_cnames_helper.push(CString::from_vec_with_nul(bytes).unwrap());
            layer_cnames.push(layer_cnames_helper.last().unwrap().as_ptr());
        }

        let dev_prop_list =
            unsafe { instance.enumerate_device_extension_properties(phy_gpu.cur_device) }?;

        let mut dev_prop_cnames = Vec::<*const libc::c_char>::new();
        let mut dev_prop_cnames_helper = Vec::<CString>::new();
        let our_dev_ext = Self::our_device_extensions(is_headless);

        for cur_ext_prop in &dev_prop_list {
            let ext_name = unsafe {
                CStr::from_ptr(cur_ext_prop.extension_name.as_ptr())
                    .to_str()
                    .unwrap()
                    .to_string()
            };
            let it = our_dev_ext.get(&ext_name);
            if let Some(str) = it {
                dev_prop_cnames_helper
                    .push(unsafe { CString::from_vec_unchecked(str.as_bytes().to_vec()) });
                dev_prop_cnames.push(dev_prop_cnames_helper.last().unwrap().as_ptr());
            }
        }

        let mut vk_queue_create_info = vk::DeviceQueueCreateInfo::default();
        vk_queue_create_info.queue_family_index = graphics_queue_index;
        vk_queue_create_info.queue_count = 1;
        let queue_prio = 1.0;
        vk_queue_create_info.p_queue_priorities = &queue_prio;
        vk_queue_create_info.flags = vk::DeviceQueueCreateFlags::default();

        let timeline_semaphore_features = vk::PhysicalDeviceTimelineSemaphoreFeatures::builder()
            .timeline_semaphore(true)
            .build();

        let mut vk_create_info = vk::DeviceCreateInfo::default();
        vk_create_info.queue_create_info_count = 1;
        vk_create_info.p_queue_create_infos = &vk_queue_create_info;
        vk_create_info.pp_enabled_extension_names = layer_cnames.as_ptr();
        vk_create_info.enabled_extension_count = layer_cnames.len() as u32;
        vk_create_info.pp_enabled_extension_names = dev_prop_cnames.as_ptr();
        vk_create_info.enabled_extension_count = dev_prop_cnames.len() as u32;
        vk_create_info.p_enabled_features = std::ptr::null();
        vk_create_info.flags = vk::DeviceCreateFlags::empty();

        if is_headless {
            vk_create_info.p_next = &timeline_semaphore_features as *const _ as *const _;
        }

        Ok(unsafe { instance.create_device(phy_gpu.cur_device, &vk_create_info, None) }?)
    }

    fn our_device_extensions(is_headless: bool) -> std::collections::BTreeSet<String> {
        let mut our_ext: std::collections::BTreeSet<String> = Default::default();
        if is_headless {
            our_ext.insert(
                vk::KhrTimelineSemaphoreFn::name()
                    .to_str()
                    .unwrap()
                    .to_string(),
            );
        } else {
            our_ext.insert(vk::KhrSwapchainFn::name().to_str().unwrap().to_string());
        }
        our_ext
    }

    #[must_use]
    pub fn final_layout(&self) -> vk::ImageLayout {
        if self.is_headless {
            vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL
        } else {
            vk::ImageLayout::PRESENT_SRC_KHR
        }
    }
}
