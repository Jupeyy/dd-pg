use std::{ffi::CStr, sync::Arc};

use ash::{prelude::VkResult, vk};
use hiarc::Hiarc;
use native::native::{
    app::{MIN_WINDOW_HEIGHT, MIN_WINDOW_WIDTH},
    PhysicalSize,
};
use raw_window_handle::{
    DisplayHandle, HasDisplayHandle, HasWindowHandle, RawDisplayHandle, WindowHandle,
};

use crate::backends::vulkan::{
    frame_resources::FrameResources, image::Image, memory::MemoryImageBlock, queue::Queue,
    surface::SurfaceKHR, swapchain::SwapchainKHR, vulkan_allocator::VulkanAllocator,
};

#[derive(Debug, Hiarc)]
pub struct BackendDisplayRequirements {
    pub extensions: Vec<String>,
    pub is_headless: bool,
}

pub enum BackendRawDisplayHandle {
    Winit { handle: RawDisplayHandle },
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
    Winit { window: &'a native::native::Window },
    Headless { width: u32, height: u32 },
}

impl<'a> BackendWindow<'a> {
    pub fn is_headless(&self) -> bool {
        match self {
            BackendWindow::Winit { .. } => false,
            BackendWindow::Headless { .. } => true,
        }
    }

    pub fn create_fake_headless_surface() -> BackendSurfaceAndHandles<'static> {
        BackendSurfaceAndHandles::Headless {
            width: 64,
            height: 64,
            surface: Default::default(),
            should_render: false,
        }
    }

    pub fn create_surface(
        &self,
        entry: &ash::Entry,
        instance: &ash::Instance,
    ) -> Result<BackendSurfaceAndHandles, vk::Result> {
        match self {
            BackendWindow::Winit { window } => {
                if let Ok((dh, wh)) = window
                    .display_handle()
                    .and_then(|dh| window.window_handle().map(|wh| (dh, wh)))
                {
                    Ok(BackendSurfaceAndHandles::Winit {
                        surface: SurfaceKHR::new(entry, instance)
                            .map_err(|_| vk::Result::ERROR_UNKNOWN)?,
                        display_handle: dh,
                        window_handle: wh,
                    })
                }
                // fall back to a headless surface
                else {
                    Ok(Self::create_fake_headless_surface())
                }
            }
            BackendWindow::Headless { width, height } => Ok(BackendSurfaceAndHandles::Headless {
                width: *width,
                height: *height,
                surface: Default::default(),
                should_render: true,
            }),
        }
    }

    pub fn inner_size(&self) -> PhysicalSize<u32> {
        match self {
            BackendWindow::Winit { window } => window.inner_size().clamp(
                PhysicalSize {
                    width: MIN_WINDOW_WIDTH,
                    height: MIN_WINDOW_HEIGHT,
                },
                PhysicalSize {
                    width: u32::MAX,
                    height: u32::MAX,
                },
            ),
            BackendWindow::Headless { width, height } => PhysicalSize::<u32>::new(*width, *height),
        }
    }

    pub fn scale_factor(&self) -> f64 {
        match self {
            BackendWindow::Winit { window } => window.scale_factor().clamp(0.0001, f64::MAX),
            BackendWindow::Headless { .. } => 1.0,
        }
    }

    fn get_window_unsafe(&self) -> &native::native::Window {
        match self {
            BackendWindow::Winit { window } => window,
            BackendWindow::Headless { .. } => {
                panic!("this function should not be called for headless clients")
            }
        }
    }
}

#[derive(Debug, Hiarc, Default)]
pub struct BackendSurfaceHeadless {
    images: Vec<(Arc<Image>, MemoryImageBlock)>,
}

impl BackendSurfaceHeadless {
    fn create_surface_images_headless(
        &mut self,
        mem_allocator: &Arc<parking_lot::Mutex<VulkanAllocator>>,
        width: u32,
        height: u32,
    ) {
        let swap_chain_count = 2;

        self.images.reserve(swap_chain_count);

        let img_format = vk::Format::B8G8R8A8_UNORM;
        (0..swap_chain_count).for_each(|_| {
            let img_res = mem_allocator.lock().create_image_ex(
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

#[derive(Debug, Hiarc)]
pub enum BackendSurfaceAndHandles<'a> {
    Winit {
        surface: Arc<SurfaceKHR>,
        #[hiarc_skip_unsafe]
        display_handle: DisplayHandle<'a>,
        #[hiarc_skip_unsafe]
        window_handle: WindowHandle<'a>,
    },
    Headless {
        width: u32,
        height: u32,
        surface: BackendSurfaceHeadless,
        /// if the headless surface was created as a result of a missing real surface
        /// it should not be rendered to if possible
        should_render: bool,
    },
}

impl<'a> BackendSurfaceAndHandles<'a> {
    /// # Safety
    /// see [`ash_window::create_surface`]
    pub unsafe fn create_vk_surface(
        self,
        entry: &ash::Entry,
        instance: &ash::Instance,
        mem_allocator: &Arc<parking_lot::Mutex<VulkanAllocator>>,
    ) -> anyhow::Result<BackendSurface> {
        match self {
            Self::Winit {
                mut surface,
                display_handle,
                window_handle,
            } => {
                let surf = ash_window::create_surface(
                    entry,
                    instance,
                    display_handle.as_raw(),
                    window_handle.as_raw(),
                    None,
                )?;
                surface = SurfaceKHR::from_existing(surface.ash_surface.clone(), surf);
                Ok(BackendSurface::Winit { surface })
            }
            Self::Headless {
                mut surface,
                width,
                height,
                should_render,
            } => {
                surface.create_surface_images_headless(mem_allocator, width, height);
                Ok(BackendSurface::Headless {
                    width,
                    height,
                    surface,
                    should_render,
                })
            }
        }
    }
}

#[derive(Debug, Hiarc)]
pub enum BackendSurface {
    Winit {
        surface: Arc<SurfaceKHR>,
    },
    Headless {
        width: u32,
        height: u32,
        surface: BackendSurfaceHeadless,
        /// if the headless surface was created as a result of a missing real surface
        /// it should not be rendered to if possible
        should_render: bool,
    },
}

impl BackendSurface {
    pub fn create_swapchain(
        &self,
        instance: &ash::Instance,
        device: &ash::Device,
        queue: &Arc<Queue>,
    ) -> Result<BackendSwapchain, vk::Result> {
        match self {
            Self::Winit { .. } => Ok(BackendSwapchain::Winit {
                swapchain: SwapchainKHR::new(instance, device)
                    .map_err(|_| vk::Result::ERROR_UNKNOWN)?,
            }),
            Self::Headless {
                surface,
                should_render,
                ..
            } => Ok(BackendSwapchain::Headless {
                images: surface
                    .images
                    .iter()
                    .map(|(img, _)| img.img(&mut FrameResources::new(None)))
                    .collect(),
                device: Arc::new(device.clone()),
                queue: queue.clone(),
                can_render: *should_render,
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
                // use build here, but make sure the lifetime is 'static
                Ok(vk::SurfaceCapabilitiesKHR::default()
                    .min_image_count(2)
                    .max_image_count(2)
                    .current_extent(ext)
                    .max_image_extent(ext)
                    .min_image_extent(ext)
                    .supported_usage_flags(
                        vk::ImageUsageFlags::COLOR_ATTACHMENT
                            | vk::ImageUsageFlags::TRANSFER_SRC
                            | vk::ImageUsageFlags::TRANSFER_DST,
                    ))
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

    pub fn can_render(&self) -> bool {
        match self {
            BackendSurface::Winit { .. } => true,
            BackendSurface::Headless { should_render, .. } => *should_render,
        }
    }

    pub fn replace(&mut self, new: Self) {
        std::mem::replace(self, new);
    }
}

#[derive(Hiarc)]
pub enum BackendSwapchain {
    Winit {
        swapchain: Arc<SwapchainKHR>,
    },
    Headless {
        #[hiarc_skip_unsafe]
        images: Vec<vk::Image>,
        #[hiarc_skip_unsafe]
        device: Arc<ash::Device>, // TODO: logical device
        queue: Arc<Queue>,
        can_render: bool,
    },
}

impl BackendSwapchain {
    pub unsafe fn queue_present(
        &self,
        queue: vk::Queue,
        present_info: vk::PresentInfoKHR,
    ) -> VkResult<bool> {
        match self {
            BackendSwapchain::Winit { swapchain } => {
                let swap_chains = [swapchain.swapchain];
                swapchain
                    .ash_swapchain
                    .queue_present(queue, &present_info.swapchains(&swap_chains))
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
            BackendSwapchain::Headless {
                device,
                queue,
                can_render,
                ..
            } => {
                // TODO: remove this wait idle call. better do it over semaphores
                let queue_guard = queue.queues.lock();
                device.device_wait_idle().unwrap();
                drop(queue_guard);
                if *can_render {
                    if semaphore != vk::Semaphore::null() {
                        let counter = device.get_semaphore_counter_value(semaphore).unwrap();
                        let signal_info = vk::SemaphoreSignalInfo::default()
                            .semaphore(semaphore)
                            .value(counter + 1);
                        device.signal_semaphore(&signal_info).unwrap();
                    }
                    if fence != vk::Fence::null() {
                        device.reset_fences(&[fence]).unwrap();
                    }
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

                let new_surface = surface.get_surface_unsafe();
                swap_info.surface = new_surface;
                if old_swap_chain.surface == new_surface {
                    swap_info.old_swapchain = old_swap_chain.swapchain;
                }

                *swapchain = SwapchainKHR::new_with_alloc(
                    old_swap_chain.ash_swapchain.clone(),
                    swap_info,
                    new_surface,
                )?;

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
