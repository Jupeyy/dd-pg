use std::fmt::Debug;

use ash::vk;
use hiarc::HiArc;
use hiarc_macro::Hiarc;

#[derive(Hiarc)]
pub struct SurfaceKHR {
    pub ash_surface: ash::extensions::khr::Surface,
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
    pub fn new(entry: &ash::Entry, instance: &ash::Instance) -> anyhow::Result<HiArc<Self>> {
        Ok(HiArc::new(Self {
            ash_surface: ash::extensions::khr::Surface::new(&entry, &instance),
            surface: Default::default(),
        }))
    }

    pub fn from_existing(
        ash_surface: ash::extensions::khr::Surface,
        surface: vk::SurfaceKHR,
    ) -> HiArc<Self> {
        HiArc::new(Self {
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
