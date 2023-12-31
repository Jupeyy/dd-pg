use std::ffi::{CStr, CString};

use anyhow::anyhow;
use ash::vk;
use config::config::EDebugGFXModes;
use hiarc::HiArc;
use hiarc_macro::hiarc;

use crate::window::BackendDisplayRequirements;

const APP_NAME: [u8; 6] = [b'D', b'D', b'N', b'e', b't', b'\0'];
const APP_VK_NAME: [u8; 13] = [
    b'D', b'D', b'N', b'e', b't', b'-', b'V', b'u', b'l', b'k', b'a', b'n', b'\0',
];

#[hiarc(1)]
#[derive(Clone)]
pub struct Instance {
    pub vk_instance: ash::Instance,
    pub vk_entry: ash::Entry,
    pub layers: Vec<String>,
}

impl std::fmt::Debug for Instance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Instance").finish()
    }
}

impl Instance {
    pub fn new(
        display_requirements: &BackendDisplayRequirements,
        dbg_mode: EDebugGFXModes,
    ) -> anyhow::Result<HiArc<Self>> {
        let entry = unsafe { ash::Entry::load() }?;

        let extensions = &display_requirements.extensions;

        let layers = Self::get_vulkan_layers(dbg_mode, &entry)?;

        let instance = Self::create_vulkan_instance(
            dbg_mode,
            &entry,
            &layers,
            extensions,
            true,
            display_requirements.is_headless,
        )?;
        Ok(HiArc::new(Self {
            vk_instance: instance,
            vk_entry: entry,
            layers,
        }))
    }

    fn our_vklayers(dbg: EDebugGFXModes) -> std::collections::BTreeSet<String> {
        let mut our_layers: std::collections::BTreeSet<String> = Default::default();

        if dbg == EDebugGFXModes::Minimum || dbg == EDebugGFXModes::All {
            our_layers.insert("VK_LAYER_KHRONOS_validation".to_string());
            // deprecated, but VK_LAYER_KHRONOS_validation was released after
            // vulkan 1.1
            our_layers.insert("VK_LAYER_LUNARG_standard_validation".to_string());
        }

        our_layers
    }

    fn get_vulkan_layers(dbg: EDebugGFXModes, entry: &ash::Entry) -> anyhow::Result<Vec<String>> {
        let vk_instance_layers = entry.enumerate_instance_layer_properties()?;

        let req_layer_names = Self::our_vklayers(dbg);
        let mut vk_layers = Vec::<String>::new();
        for layer_name in &vk_instance_layers {
            let layer_name = unsafe {
                CStr::from_ptr(layer_name.layer_name.as_ptr())
                    .to_str()
                    .unwrap()
                    .to_string()
            };
            let it = req_layer_names.get(&layer_name);
            if let Some(_layer) = it {
                vk_layers.push(layer_name);
            }
        }

        Ok(vk_layers)
    }

    fn create_vulkan_instance(
        dbg: EDebugGFXModes,
        entry: &ash::Entry,
        vk_layers: &Vec<String>,
        vk_extensions: &Vec<String>,
        try_debug_extensions: bool,
        is_headless: bool,
    ) -> anyhow::Result<ash::Instance> {
        let mut layers_cstr: Vec<*const libc::c_char> = Default::default();
        let mut layers_cstr_helper: Vec<CString> = Default::default();
        layers_cstr.reserve(vk_layers.len());
        for layer in vk_layers {
            layers_cstr_helper
                .push(unsafe { CString::from_vec_unchecked(layer.as_bytes().to_vec()) });
            layers_cstr.push(layers_cstr_helper.last().unwrap().as_ptr());
        }

        let mut ext_cstr: Vec<*const libc::c_char> = Default::default();
        let mut ext_cstr_helper: Vec<CString> = Default::default();
        ext_cstr.reserve(vk_extensions.len() + 1);
        for ext in vk_extensions {
            ext_cstr_helper.push(unsafe { CString::from_vec_unchecked(ext.as_bytes().to_vec()) });
            ext_cstr.push(ext_cstr_helper.last().unwrap().as_ptr());
        }

        if try_debug_extensions && (dbg == EDebugGFXModes::Minimum || dbg == EDebugGFXModes::All) {
            // debug message support
            ext_cstr.push(vk::ExtDebugUtilsFn::name().as_ptr());
        }

        let mut vk_app_info = vk::ApplicationInfo::default();
        vk_app_info.p_application_name = APP_NAME.as_ptr() as *const i8;
        vk_app_info.application_version = 1;
        vk_app_info.p_engine_name = APP_VK_NAME.as_ptr() as *const i8;
        vk_app_info.engine_version = 1;
        vk_app_info.api_version = if is_headless {
            vk::API_VERSION_1_2
        } else {
            vk::API_VERSION_1_1
        };

        let mut ptr_ext = std::ptr::null();
        let mut features = vk::ValidationFeaturesEXT::default();
        let enabled_exts = [
            vk::ValidationFeatureEnableEXT::SYNCHRONIZATION_VALIDATION,
            vk::ValidationFeatureEnableEXT::BEST_PRACTICES,
        ];
        if try_debug_extensions
            && (dbg == EDebugGFXModes::AffectsPerformance || dbg == EDebugGFXModes::All)
        {
            features.enabled_validation_feature_count = enabled_exts.len() as u32;
            features.p_enabled_validation_features = enabled_exts.as_ptr();

            ptr_ext = &features;
        }

        let mut vk_instance_info = vk::InstanceCreateInfo::default();
        vk_instance_info.p_next = ptr_ext as *const libc::c_void;
        vk_instance_info.flags = vk::InstanceCreateFlags::empty();
        vk_instance_info.p_application_info = &vk_app_info;
        vk_instance_info.enabled_extension_count = ext_cstr.len() as u32;
        vk_instance_info.pp_enabled_extension_names = ext_cstr.as_ptr();
        vk_instance_info.enabled_layer_count = layers_cstr.len() as u32;
        vk_instance_info.pp_enabled_layer_names = layers_cstr.as_ptr();

        let mut try_again: bool = false;

        let res = unsafe { entry.create_instance(&vk_instance_info, None) };
        if let Err(res_err) = res {
            if res_err == vk::Result::ERROR_LAYER_NOT_PRESENT
                || res_err == vk::Result::ERROR_EXTENSION_NOT_PRESENT
            {
                try_again = true;
            } else {
                return Err(anyhow!("Creating instance failed: {res_err}"));
            }
        }

        if try_again && try_debug_extensions {
            return Self::create_vulkan_instance(
                dbg,
                entry,
                vk_layers,
                vk_extensions,
                false,
                is_headless,
            );
        }

        Ok(res.unwrap())
    }
}

impl Drop for Instance {
    fn drop(&mut self) {
        unsafe { self.vk_instance.destroy_instance(None) };
    }
}
