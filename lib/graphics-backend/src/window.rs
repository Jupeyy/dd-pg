use std::ffi::CStr;

use ash::{prelude::VkResult, vk};
use hiarc::HiArc;
use raw_window_handle::{HasRawDisplayHandle, HasRawWindowHandle};
use winit::dpi::PhysicalSize;

use crate::backends::vulkan::{
    frame_resources::FrameResources, image::Image, memory::MemoryImageBlock, queue::Queue,
    surface::SurfaceKHR, swapchain::SwapchainKHR, vulkan_device::Device,
};

pub struct BackendDisplayRequirements {
    pub extensions: Vec<String>,
    pub is_headless: bool,
}

pub enum BackendRawDisplayHandle {
    Winit {
        handle: winit::window::raw_window_handle::RawDisplayHandle,
    },
    Headless,
}

impl BackendRawDisplayHandle {
    pub fn enumerate_required_vk_extensions(&self) -> Result<Vec<String>, vk::Result> {
        match self {
            Self::Winit { handle } => {
                let mut vk_extensions = Vec::<String>::new();
                let ext_list = ash_window::enumerate_required_extensions(*handle)?;

                for ext in ext_list {
                    let ext_name = unsafe { CStr::from_ptr(*ext).to_str().unwrap().to_string() };
                    vk_extensions.push(ext_name);
                }

                Ok(vk_extensions)
            }
            Self::Headless => Ok(Default::default()),
        }
    }

    pub fn is_headless(&self) -> bool {
        match self {
            Self::Winit { .. } => false,
            Self::Headless => true,
        }
    }
}

pub enum BackendWindow<'a> {
    Winit { window: &'a winit::window::Window },
    Headless { width: u32, height: u32 },
}

impl<'a> BackendWindow<'a> {
    pub fn is_headless(&self) -> bool {
        match self {
            BackendWindow::Winit { .. } => false,
            BackendWindow::Headless { .. } => true,
        }
    }

    pub fn create_surface(
        &self,
        entry: &ash::Entry,
        instance: &ash::Instance,
    ) -> Result<BackendSurface, vk::Result> {
        match self {
            BackendWindow::Winit { .. } => Ok(BackendSurface::Winit {
                surface: SurfaceKHR::new(&entry, &instance)
                    .map_err(|_| vk::Result::ERROR_UNKNOWN)?,
            }),
            BackendWindow::Headless { width, height } => Ok(BackendSurface::Headless {
                width: *width,
                height: *height,
                surface: Default::default(),
            }),
        }
    }

    pub fn inner_size(&self) -> PhysicalSize<u32> {
        match self {
            BackendWindow::Winit { window } => window.inner_size(),
            BackendWindow::Headless { width, height } => PhysicalSize::<u32>::new(*width, *height),
        }
    }

    pub fn scale_factor(&self) -> f64 {
        match self {
            BackendWindow::Winit { window } => window.scale_factor(),
            BackendWindow::Headless { .. } => 1.0,
        }
    }

    fn get_window_unsafe(&self) -> &winit::window::Window {
        match self {
            BackendWindow::Winit { window } => window,
            BackendWindow::Headless { .. } => {
                panic!("this function should not be called for headless clients")
            }
        }
    }
}

#[derive(Debug, Default)]
pub struct BackendSurfaceHeadless {
    images: Vec<(HiArc<Image>, MemoryImageBlock)>,
}

impl BackendSurfaceHeadless {
    fn create_surface_images_headless(&mut self, device: &Device, width: u32, height: u32) {
        let swap_chain_count = 2;

        self.images.reserve(swap_chain_count);

        let img_format = vk::Format::B8G8R8A8_UNORM;
        (0..swap_chain_count).for_each(|_| {
            let img_res = device.mem_allocator.lock().create_image_ex(
                width,
                height,
                1,
                1,
                img_format,
                vk::ImageTiling::OPTIMAL,
                vk::ImageUsageFlags::COLOR_ATTACHMENT
                    | vk::ImageUsageFlags::INPUT_ATTACHMENT
                    | vk::ImageUsageFlags::SAMPLED
                    | vk::ImageUsageFlags::TRANSFER_SRC
                    | vk::ImageUsageFlags::TRANSFER_DST,
                None,
            );
            if img_res.is_err() {
                panic!("failed to allocate images");
            }

            let (img, img_mem) = img_res.unwrap();

            self.images.push((img, img_mem));
        });
    }
}

pub enum BackendSurface {
    Winit {
        surface: HiArc<SurfaceKHR>,
    },
    Headless {
        width: u32,
        height: u32,
        surface: BackendSurfaceHeadless,
    },
}

impl BackendSurface {
    pub unsafe fn create_vk_surface(
        &mut self,
        entry: &ash::Entry,
        instance: &ash::Instance,
        window: &BackendWindow,
        device: &Device,
    ) -> Result<(), vk::Result> {
        match self {
            BackendSurface::Winit { surface, .. } => {
                let surf = ash_window::create_surface(
                    entry,
                    instance,
                    window.get_window_unsafe().raw_display_handle(),
                    window.get_window_unsafe().raw_window_handle(),
                    None,
                )?;
                *surface = SurfaceKHR::from_existing(surface.ash_surface.clone(), surf);
                Ok(())
            }
            BackendSurface::Headless {
                surface,
                width,
                height,
            } => Ok(surface.create_surface_images_headless(device, *width, *height)),
        }
    }

    pub fn create_swapchain(
        &self,
        instance: &ash::Instance,
        device: &ash::Device,
        queue: &HiArc<Queue>,
    ) -> Result<BackendSwapchain, vk::Result> {
        match self {
            Self::Winit { .. } => Ok(BackendSwapchain::Winit {
                swapchain: SwapchainKHR::new(instance, device)
                    .map_err(|_| vk::Result::ERROR_UNKNOWN)?,
            }),
            Self::Headless { surface, .. } => Ok(BackendSwapchain::Headless {
                images: surface
                    .images
                    .iter()
                    .map(|(img, _)| img.inner_arc().img(&mut FrameResources::new(None)))
                    .collect(),
                device: device.clone(),
                queue: queue.clone(),
            }),
        }
    }

    pub unsafe fn get_physical_device_surface_support(
        &self,
        physical_device: vk::PhysicalDevice,
        queue_family_index: u32,
    ) -> VkResult<bool> {
        match self {
            BackendSurface::Winit { surface } => {
                surface.ash_surface.get_physical_device_surface_support(
                    physical_device,
                    queue_family_index,
                    surface.surface,
                )
            }
            BackendSurface::Headless { .. } => Ok(true),
        }
    }

    pub unsafe fn get_physical_device_surface_formats(
        &self,
        physical_device: vk::PhysicalDevice,
    ) -> VkResult<Vec<vk::SurfaceFormatKHR>> {
        match self {
            BackendSurface::Winit { surface } => surface
                .ash_surface
                .get_physical_device_surface_formats(physical_device, surface.surface),
            BackendSurface::Headless { .. } => Ok(vec![vk::SurfaceFormatKHR {
                format: vk::Format::B8G8R8A8_UNORM,
                color_space: vk::ColorSpaceKHR::SRGB_NONLINEAR,
            }]),
        }
    }

    pub unsafe fn get_physical_device_surface_present_modes(
        &self,
        physical_device: vk::PhysicalDevice,
    ) -> VkResult<Vec<vk::PresentModeKHR>> {
        match self {
            BackendSurface::Winit { surface } => surface
                .ash_surface
                .get_physical_device_surface_present_modes(physical_device, surface.surface),
            BackendSurface::Headless { .. } => Ok(vec![vk::PresentModeKHR::IMMEDIATE]),
        }
    }

    pub unsafe fn get_physical_device_surface_capabilities(
        &self,
        physical_device: vk::PhysicalDevice,
    ) -> VkResult<vk::SurfaceCapabilitiesKHR> {
        match self {
            BackendSurface::Winit { surface } => surface
                .ash_surface
                .get_physical_device_surface_capabilities(physical_device, surface.surface),
            BackendSurface::Headless { width, height, .. } => {
                let ext = vk::Extent2D {
                    width: *width,
                    height: *height,
                };
                Ok(vk::SurfaceCapabilitiesKHR::builder()
                    .min_image_count(2)
                    .max_image_count(2)
                    .current_extent(ext)
                    .max_image_extent(ext)
                    .min_image_extent(ext)
                    .supported_usage_flags(
                        vk::ImageUsageFlags::COLOR_ATTACHMENT
                            | vk::ImageUsageFlags::TRANSFER_SRC
                            | vk::ImageUsageFlags::TRANSFER_DST,
                    )
                    .build())
            }
        }
    }

    fn get_surface_unsafe(&self) -> vk::SurfaceKHR {
        match self {
            BackendSurface::Winit { surface, .. } => surface.surface,
            BackendSurface::Headless { .. } => {
                panic!("this function should not be called for headless clients")
            }
        }
    }
}

pub enum BackendSwapchain {
    Winit {
        swapchain: HiArc<SwapchainKHR>,
    },
    Headless {
        images: Vec<vk::Image>,
        device: ash::Device, // TODO: logical device
        queue: HiArc<Queue>,
    },
}

impl BackendSwapchain {
    pub unsafe fn queue_present(
        &self,
        queue: vk::Queue,
        mut present_info: vk::PresentInfoKHR,
    ) -> VkResult<bool> {
        match self {
            BackendSwapchain::Winit { swapchain } => {
                let swap_chains = [swapchain.swapchain];
                present_info.swapchain_count = swap_chains.len() as u32;
                present_info.p_swapchains = swap_chains.as_ptr();
                swapchain.ash_swapchain.queue_present(queue, &present_info)
            }
            BackendSwapchain::Headless { .. } => Ok(false),
        }
    }

    pub unsafe fn acquire_next_image(
        &self,
        timeout: u64,
        semaphore: vk::Semaphore,
        fence: vk::Fence,
    ) -> VkResult<(u32, bool)> {
        match self {
            BackendSwapchain::Winit { swapchain } => swapchain.ash_swapchain.acquire_next_image(
                swapchain.swapchain,
                timeout,
                semaphore,
                fence,
            ),
            BackendSwapchain::Headless { device, queue, .. } => {
                // TODO: remove this wait idle call. better do it over semaphores
                let queue_guard = queue.queues.lock();
                device.device_wait_idle().unwrap();
                drop(queue_guard);
                if semaphore != vk::Semaphore::null() {
                    let counter = device.get_semaphore_counter_value(semaphore).unwrap();
                    let signal_info = vk::SemaphoreSignalInfo::builder()
                        .semaphore(semaphore)
                        .value(counter + 1)
                        .build();
                    device.signal_semaphore(&signal_info).unwrap();
                }
                if fence != vk::Fence::null() {
                    device.reset_fences(&[fence]).unwrap();
                }
                Ok((0, false))
            }
        }
    }

    pub unsafe fn create_swapchain(
        &mut self,
        surface: &BackendSurface,
        mut swap_info: vk::SwapchainCreateInfoKHR,
    ) -> VkResult<()> {
        match self {
            BackendSwapchain::Winit { swapchain } => {
                let old_swap_chain = swapchain.clone();

                swap_info.surface = surface.get_surface_unsafe();
                swap_info.old_swapchain = old_swap_chain.swapchain;

                *swapchain =
                    SwapchainKHR::new_with_alloc(old_swap_chain.ash_swapchain.clone(), swap_info)?;

                Ok(())
            }
            BackendSwapchain::Headless { .. } => Ok(()),
        }
    }

    pub unsafe fn get_swapchain_images(&self) -> VkResult<Vec<vk::Image>> {
        match self {
            BackendSwapchain::Winit { swapchain } => swapchain
                .ash_swapchain
                .get_swapchain_images(swapchain.swapchain),
            BackendSwapchain::Headless { images, .. } => Ok(images.clone()),
        }
    }
}
