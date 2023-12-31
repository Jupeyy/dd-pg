use std::fmt::Debug;

use ash::vk;
use hiarc::HiArc;
use hiarc_macro::Hiarc;

#[derive(Hiarc)]
pub struct DebugUtilsMessengerEXT {
    debug_messenger: vk::DebugUtilsMessengerEXT,
    dbg_utils: ash::extensions::ext::DebugUtils,
}

impl Debug for DebugUtilsMessengerEXT {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DebugUtilsMessengerEXT")
            .field("debug_messenger", &self.debug_messenger)
            .finish()
    }
}

impl DebugUtilsMessengerEXT {
    pub fn new(
        entry: &ash::Entry,
        instance: &ash::Instance,
        create_info: &vk::DebugUtilsMessengerCreateInfoEXT,
    ) -> anyhow::Result<HiArc<Self>> {
        let dbg_utils = ash::extensions::ext::DebugUtils::new(entry, instance);
        let debug_messenger = unsafe { dbg_utils.create_debug_utils_messenger(create_info, None) }?;
        Ok(HiArc::new(Self {
            debug_messenger,
            dbg_utils,
        }))
    }
}

impl Drop for DebugUtilsMessengerEXT {
    fn drop(&mut self) {
        unsafe {
            self.dbg_utils
                .destroy_debug_utils_messenger(self.debug_messenger, None)
        };
    }
}
