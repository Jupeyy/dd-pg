use std::{fmt::Debug, sync::Arc};

use ash::vk;
use hiarc::Hiarc;

#[derive(Hiarc)]
pub struct SurfaceKHR {
    #[hiarc_skip_unsafe]
    pub ash_surface: ash::khr::surface::Instance,
    #[hiarc_skip_unsafe]
    pub surface: vk::SurfaceKHR,
}

impl Debug for SurfaceKHR {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SurfaceKHR")
            .field("surface", &self.surface)
            .finish()
    }
}

impl SurfaceKHR {
    pub fn new(entry: &ash::Entry, instance: &ash::Instance) -> anyhow::Result<Arc<Self>> {
        Ok(Arc::new(Self {
            ash_surface: ash::khr::surface::Instance::new(entry, instance),
            surface: Default::default(),
        }))
    }

    pub fn from_existing(
        ash_surface: ash::khr::surface::Instance,
        surface: vk::SurfaceKHR,
    ) -> Arc<Self> {
        Arc::new(Self {
            ash_surface,
            surface,
        })
    }
}

impl Drop for SurfaceKHR {
    fn drop(&mut self) {
        if self.surface != vk::SurfaceKHR::null() {
            unsafe {
                self.ash_surface.destroy_surface(self.surface, None);
            }
        }
    }
}
