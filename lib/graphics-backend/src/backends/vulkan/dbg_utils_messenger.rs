use std::{fmt::Debug, sync::Arc};

use ash::vk;
use hiarc::Hiarc;

#[derive(Hiarc)]
pub struct DebugUtilsMessengerEXT {
    #[hiarc_skip_unsafe]
    debug_messenger: vk::DebugUtilsMessengerEXT,
    #[hiarc_skip_unsafe]
    dbg_utils: ash::ext::debug_utils::Instance,
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
    ) -> anyhow::Result<Arc<Self>> {
        let dbg_utils = ash::ext::debug_utils::Instance::new(entry, instance);
        let debug_messenger = unsafe { dbg_utils.create_debug_utils_messenger(create_info, None) }?;
        Ok(Arc::new(Self {
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
