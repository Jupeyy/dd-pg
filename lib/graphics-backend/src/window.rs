use ash::{prelude::VkResult, vk};
use raw_window_handle::{HasRawDisplayHandle, HasRawWindowHandle};
use winit::dpi::PhysicalSize;

use crate::backends::vulkan::{
    vulkan_device::Device,
    vulkan_types::{CTexture, SMemoryImageBlock, IMAGE_BUFFER_CACHE_ID},
};

pub enum BackendWindow<'a> {
    Winit { window: &'a winit::window::Window },
    Headless { width: u32, height: u32 },
}

impl<'a> BackendWindow<'a> {
    pub fn enumerate_required_vk_extensions(&self) -> Result<&[*const i8], vk::Result> {
        match self {
            BackendWindow::Winit { window } => {
                ash_window::enumerate_required_extensions(window.raw_display_handle())
            }
            BackendWindow::Headless { .. } => Ok(&[]),
        }
    }

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
                surface: vk::SurfaceKHR::null(),
                ash_surface: ash::extensions::khr::Surface::new(&entry, &instance),
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
    images: Vec<CTexture>,
}

impl BackendSurfaceHeadless {
    fn create_surface_images_headless(&mut self, device: &mut Device, width: u32, height: u32) {
        let swap_chain_count = 2;

        self.images.resize(swap_chain_count, Default::default());

        let img_format = vk::Format::B8G8R8A8_UNORM;
        self.images.iter_mut().for_each(|img_second_pass| {
            let mut img = vk::Image::default();
            let mut img_mem = SMemoryImageBlock::<IMAGE_BUFFER_CACHE_ID>::default();
            if !device.create_image_ex(
                width,
                height,
                1,
                1,
                img_format,
                vk::ImageTiling::OPTIMAL,
                &mut img,
                &mut img_mem,
                vk::ImageUsageFlags::COLOR_ATTACHMENT
                    | vk::ImageUsageFlags::INPUT_ATTACHMENT
                    | vk::ImageUsageFlags::SAMPLED
                    | vk::ImageUsageFlags::TRANSFER_SRC,
                None,
                0,
            ) {
                panic!("failed to allocate images");
            }

            img_second_pass.img = img;
            img_second_pass.img_mem = img_mem;
        });
    }

    unsafe fn destroy_surface_images_headless(&mut self, device: &mut Device) {
        self.images.iter_mut().for_each(|img| {
            device.ash_vk.device.destroy_image(img.img, None);

            Device::free_image_mem_block(
                &mut device.frame_delayed_buffer_cleanups,
                &mut device.image_buffer_caches,
                &mut img.img_mem,
                0,
            );
        });

        self.images.clear()
    }
}

pub enum BackendSurface {
    Winit {
        ash_surface: ash::extensions::khr::Surface,
        surface: vk::SurfaceKHR,
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
        device: &mut Device,
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
                *surface = surf;
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
    ) -> Result<BackendSwapchain, vk::Result> {
        match self {
            Self::Winit { .. } => Ok(BackendSwapchain::Winit {
                swapchain: vk::SwapchainKHR::null(),
                ash_swapchain: ash::extensions::khr::Swapchain::new(instance, device),
            }),
            Self::Headless { surface, .. } => Ok(BackendSwapchain::Headless {
                images: surface.images.iter().map(|img| img.img).collect(),
                device: device.clone(),
            }),
        }
    }

    pub unsafe fn destroy_vk_surface(&mut self, device: &mut Device) {
        match self {
            BackendSurface::Winit {
                ash_surface,
                surface,
            } => ash_surface.destroy_surface(*surface, None),
            BackendSurface::Headless { surface, .. } => {
                surface.destroy_surface_images_headless(device)
            }
        }
    }

    pub unsafe fn get_physical_device_surface_support(
        &self,
        physical_device: vk::PhysicalDevice,
        queue_family_index: u32,
    ) -> VkResult<bool> {
        match self {
            BackendSurface::Winit {
                ash_surface,
                surface,
            } => ash_surface.get_physical_device_surface_support(
                physical_device,
                queue_family_index,
                *surface,
            ),
            BackendSurface::Headless { .. } => Ok(true),
        }
    }

    pub unsafe fn get_physical_device_surface_formats(
        &self,
        physical_device: vk::PhysicalDevice,
    ) -> VkResult<Vec<vk::SurfaceFormatKHR>> {
        match self {
            BackendSurface::Winit {
                ash_surface,
                surface,
            } => ash_surface.get_physical_device_surface_formats(physical_device, *surface),
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
            BackendSurface::Winit {
                ash_surface,
                surface,
            } => ash_surface.get_physical_device_surface_present_modes(physical_device, *surface),
            BackendSurface::Headless { .. } => Ok(vec![vk::PresentModeKHR::IMMEDIATE]),
        }
    }

    pub unsafe fn get_physical_device_surface_capabilities(
        &self,
        physical_device: vk::PhysicalDevice,
    ) -> VkResult<vk::SurfaceCapabilitiesKHR> {
        match self {
            BackendSurface::Winit {
                ash_surface,
                surface,
            } => ash_surface.get_physical_device_surface_capabilities(physical_device, *surface),
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
                        vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::TRANSFER_SRC,
                    )
                    .build())
            }
        }
    }

    fn get_surface_unsafe(&self) -> vk::SurfaceKHR {
        match self {
            BackendSurface::Winit { surface, .. } => *surface,
            BackendSurface::Headless { .. } => {
                panic!("this function should not be called for headless clients")
            }
        }
    }
}

pub enum BackendSwapchain {
    Winit {
        ash_swapchain: ash::extensions::khr::Swapchain,
        swapchain: vk::SwapchainKHR,
    },
    Headless {
        images: Vec<vk::Image>,
        device: ash::Device,
    },
}

impl BackendSwapchain {
    pub unsafe fn queue_present(
        &self,
        queue: vk::Queue,
        mut present_info: vk::PresentInfoKHR,
    ) -> VkResult<bool> {
        match self {
            BackendSwapchain::Winit {
                ash_swapchain,
                swapchain,
            } => {
                let swap_chains = [*swapchain];
                present_info.swapchain_count = swap_chains.len() as u32;
                present_info.p_swapchains = swap_chains.as_ptr();
                ash_swapchain.queue_present(queue, &present_info)
            }
            BackendSwapchain::Headless { .. } => Ok(true),
        }
    }

    pub unsafe fn acquire_next_image(
        &self,
        timeout: u64,
        semaphore: vk::Semaphore,
        fence: vk::Fence,
    ) -> VkResult<(u32, bool)> {
        match self {
            BackendSwapchain::Winit {
                ash_swapchain,
                swapchain,
            } => ash_swapchain.acquire_next_image(*swapchain, timeout, semaphore, fence),
            BackendSwapchain::Headless { device, .. } => {
                device.device_wait_idle().unwrap();
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

    pub unsafe fn destroy_swapchain(&mut self) {
        match self {
            BackendSwapchain::Winit {
                ash_swapchain,
                swapchain,
            } => {
                ash_swapchain.destroy_swapchain(*swapchain, None);
                *swapchain = vk::SwapchainKHR::null();
            }
            BackendSwapchain::Headless { .. } => {}
        }
    }

    pub unsafe fn create_swapchain(
        &mut self,
        surface: &BackendSurface,
        mut swap_info: vk::SwapchainCreateInfoKHR,
    ) -> VkResult<vk::SwapchainKHR> {
        match self {
            BackendSwapchain::Winit {
                ash_swapchain,
                swapchain,
            } => {
                let old_swap_chain = *swapchain;

                swap_info.surface = surface.get_surface_unsafe();
                swap_info.old_swapchain = old_swap_chain;

                *swapchain = vk::SwapchainKHR::default();
                *swapchain = ash_swapchain.create_swapchain(&swap_info, None)?;
                Ok(old_swap_chain)
            }
            BackendSwapchain::Headless { .. } => Ok(vk::SwapchainKHR::null()),
        }
    }

    pub unsafe fn get_swapchain_images(&self) -> VkResult<Vec<vk::Image>> {
        match self {
            BackendSwapchain::Winit {
                ash_swapchain,
                swapchain,
            } => ash_swapchain.get_swapchain_images(*swapchain),
            BackendSwapchain::Headless { images, .. } => Ok(images.clone()),
        }
    }
}
