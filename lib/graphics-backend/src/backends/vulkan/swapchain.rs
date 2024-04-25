use std::{fmt::Debug, sync::Arc};

use anyhow::anyhow;
use ash::vk;
use base_log::log::{LogLevel, SystemLogGroup, SystemLogInterface};
use config::config::AtomicGFXDebugModes;
use hiarc::Hiarc;

use crate::window::{BackendSurface, BackendSwapchain};

use super::{
    image::Image, logical_device::LogicalDevice, phy_device::PhyDevice, vulkan_dbg::is_verbose,
};

/// this is basically the swapchain frontent to the swapchain backend
/// it might know about engine realted types and isn't really a pure
/// vulkan type wrapper
#[derive(Debug, Hiarc)]
pub struct Swapchain {
    #[hiarc_skip_unsafe]
    pub extent: vk::Extent2D,
    #[hiarc_skip_unsafe]
    pub format: vk::SurfaceFormatKHR,
}

pub struct SwapchainCreateOptions {
    pub vsync: bool,
}

impl Swapchain {
    fn get_surface_properties(
        phy_device: &PhyDevice,
        surface: &BackendSurface,
    ) -> anyhow::Result<vk::SurfaceCapabilitiesKHR> {
        Ok(unsafe { surface.get_physical_device_surface_capabilities(phy_device.cur_device) }?)
    }

    fn get_presentation_mode(
        phy_device: &PhyDevice,
        surface: &BackendSurface,
        logger: &SystemLogGroup,
        options: &SwapchainCreateOptions,
    ) -> anyhow::Result<vk::PresentModeKHR> {
        let present_mode_list =
            unsafe { surface.get_physical_device_surface_present_modes(phy_device.cur_device) }
                .map_err(|err| {
                    anyhow!("get_physical_device_surface_present_modes failed: {err}")
                })?;

        let mut vk_io_mode = if options.vsync {
            vk::PresentModeKHR::FIFO
        } else {
            vk::PresentModeKHR::IMMEDIATE
        };
        for mode in &present_mode_list {
            if *mode == vk_io_mode {
                return Ok(vk_io_mode);
            }
        }

        // TODO dbg_msg("vulkan", "warning: requested presentation mode was not available. falling back to mailbox / fifo relaxed.");
        vk_io_mode = if options.vsync {
            vk::PresentModeKHR::FIFO_RELAXED
        } else {
            vk::PresentModeKHR::MAILBOX
        };
        for mode in &present_mode_list {
            if *mode == vk_io_mode {
                return Ok(vk_io_mode);
            }
        }

        logger
            .log(LogLevel::Warning)
            .msg("requested presentation mode was not available. using first available.");
        if !present_mode_list.is_empty() {
            vk_io_mode = present_mode_list[0];
        } else {
            return Err(anyhow!("List of presentation modes was empty."));
        }

        Ok(vk_io_mode)
    }

    fn get_number_of_swap_images(
        logger: &SystemLogGroup,
        dbg: &AtomicGFXDebugModes,
        vk_capabilities: &vk::SurfaceCapabilitiesKHR,
    ) -> u32 {
        let img_number = vk_capabilities.min_image_count + 1;
        if is_verbose(dbg) {
            logger
                .log(LogLevel::Debug)
                .msg("minimal swap image count ")
                .msg_var(&vk_capabilities.min_image_count);
        }
        if vk_capabilities.max_image_count > 0 && img_number > vk_capabilities.max_image_count {
            vk_capabilities.max_image_count
        } else {
            img_number
        }
    }

    fn get_swap_image_size(
        canvas_size: (u32, u32),
        vk_capabilities: &vk::SurfaceCapabilitiesKHR,
    ) -> vk::Extent2D {
        let mut ret_size = vk::Extent2D {
            width: canvas_size.0,
            height: canvas_size.1,
        };

        if vk_capabilities.current_extent.width == u32::MAX {
            ret_size.width = ret_size.width.clamp(
                vk_capabilities.min_image_extent.width,
                vk_capabilities.max_image_extent.width,
            );
            ret_size.height = ret_size.height.clamp(
                vk_capabilities.min_image_extent.height,
                vk_capabilities.max_image_extent.height,
            );
        } else {
            ret_size = vk_capabilities.current_extent;
        }

        ret_size
    }

    fn our_image_usages() -> Vec<vk::ImageUsageFlags> {
        let mut img_usages: Vec<vk::ImageUsageFlags> = Default::default();

        img_usages.push(vk::ImageUsageFlags::COLOR_ATTACHMENT);
        img_usages.push(vk::ImageUsageFlags::TRANSFER_SRC);
        img_usages.push(vk::ImageUsageFlags::TRANSFER_DST);

        img_usages
    }

    fn get_image_usage(
        vk_capabilities: &vk::SurfaceCapabilitiesKHR,
    ) -> anyhow::Result<vk::ImageUsageFlags> {
        let our_img_usages = Self::our_image_usages();
        assert!(!our_img_usages.is_empty());

        let mut res = our_img_usages[0];

        for img_usage in &our_img_usages {
            let img_usage_flags = *img_usage & vk_capabilities.supported_usage_flags;
            if img_usage_flags != *img_usage {
                return Err(anyhow!(
                    "Framebuffer image attachment types not supported. (supported: {:?})",
                    vk_capabilities.supported_usage_flags
                ));
            }

            res |= *img_usage;
        }

        Ok(res)
    }

    fn get_transform(vk_capabilities: &vk::SurfaceCapabilitiesKHR) -> vk::SurfaceTransformFlagsKHR {
        if !(vk_capabilities.supported_transforms & vk::SurfaceTransformFlagsKHR::IDENTITY)
            .is_empty()
        {
            return vk::SurfaceTransformFlagsKHR::IDENTITY;
        }
        vk_capabilities.current_transform
    }

    fn get_format(
        phy_device: &PhyDevice,
        surface: &BackendSurface,
    ) -> anyhow::Result<vk::SurfaceFormatKHR> {
        let _surf_formats: u32 = 0;
        let surf_format_list = unsafe {
            surface
                .get_physical_device_surface_formats(phy_device.cur_device)
        }.map_err(|err| {
            if  err != vk::Result::INCOMPLETE {
                anyhow!("The device surface format fetching failed.")
            }
            else {
                anyhow!("warning: not all surface formats are requestable with your current settings.\nThe device surface format fetching failed.")
            }
        })?;

        if surf_format_list.len() == 1 && surf_format_list[0].format == vk::Format::UNDEFINED {
            // TODO dbg_msg("vulkan", "warning: surface format was undefined. This can potentially cause bugs.");
            return Ok(vk::SurfaceFormatKHR::default()
                .format(vk::Format::B8G8R8A8_UNORM)
                .color_space(vk::ColorSpaceKHR::SRGB_NONLINEAR));
        }

        for find_format in &surf_format_list {
            if (find_format.format == vk::Format::B8G8R8A8_UNORM
                && find_format.color_space == vk::ColorSpaceKHR::SRGB_NONLINEAR)
                || (find_format.format == vk::Format::R8G8B8A8_UNORM
                    && find_format.color_space == vk::ColorSpaceKHR::SRGB_NONLINEAR)
            {
                return Ok(*find_format);
            }
        }

        // TODO dbg_msg("vulkan", "warning: surface format was not RGBA(or variants of it). This can potentially cause weird looking images(too bright etc.).");
        Ok(surf_format_list[0])
    }

    fn create_swap_chain(
        phy_device: &PhyDevice,
        surface: &BackendSurface,
        swapchain: &mut BackendSwapchain,
        options: &SwapchainCreateOptions,
        logger: &SystemLogGroup,
        dbg: &AtomicGFXDebugModes,
        canvas_size: (u32, u32),
    ) -> anyhow::Result<(vk::Extent2D, vk::SurfaceFormatKHR)> {
        let vksurf_cap = Self::get_surface_properties(phy_device, surface)
            .map_err(|err| anyhow!("Could not get surface properties: {err}"))?;

        let present_mode = Self::get_presentation_mode(phy_device, surface, logger, options)?;

        let swap_img_count = Self::get_number_of_swap_images(logger, dbg, &vksurf_cap);

        let swap_chain_extent = Self::get_swap_image_size(canvas_size, &vksurf_cap);

        let usage_flags = Self::get_image_usage(&vksurf_cap)?;

        let transform_flag_bits = Self::get_transform(&vksurf_cap);

        let surf_format = Self::get_format(phy_device, surface)?;

        let mut swap_info = vk::SwapchainCreateInfoKHR::default();
        swap_info.flags = vk::SwapchainCreateFlagsKHR::empty();

        swap_info.min_image_count = swap_img_count;
        swap_info.image_format = surf_format.format;
        swap_info.image_color_space = surf_format.color_space;
        swap_info.image_extent = swap_chain_extent;
        swap_info.image_array_layers = 1;
        swap_info.image_usage = usage_flags;
        swap_info.image_sharing_mode = vk::SharingMode::EXCLUSIVE;
        swap_info.pre_transform = transform_flag_bits;
        swap_info.composite_alpha = vk::CompositeAlphaFlagsKHR::OPAQUE;
        swap_info.present_mode = present_mode;
        swap_info.clipped = vk::TRUE;

        unsafe { swapchain.create_swapchain(surface, swap_info) }
            .map_err(|err| anyhow!("Creating the swap chain failed: {err}"))?;

        Ok((swap_chain_extent, surf_format))
    }

    pub fn get_swap_chain_image_handles(
        swapchain: &BackendSwapchain,
        device: &Arc<LogicalDevice>,
    ) -> anyhow::Result<Vec<Arc<Image>>> {
        let mut swap_chain_images = unsafe { swapchain.get_swapchain_images() }
            .map_err(|err| anyhow!("Could not get swap chain images: {err}"))?;

        let images: Vec<Arc<Image>> = swap_chain_images
            .drain(..)
            .map(|img| Image::from_external_resource(device.clone(), img))
            .collect();

        Ok(images)
    }

    pub fn new(
        phy_device: &PhyDevice,
        surface: &BackendSurface,
        swapchain: &mut BackendSwapchain,
        options: &SwapchainCreateOptions,
        logger: &SystemLogGroup,
        dbg: &AtomicGFXDebugModes,
        canvas_size: (u32, u32),
    ) -> anyhow::Result<Self> {
        let (extent, format) = Self::create_swap_chain(
            phy_device,
            surface,
            swapchain,
            options,
            logger,
            dbg,
            canvas_size,
        )?;

        Ok(Self { extent, format })
    }
}

#[derive(Hiarc)]
pub struct SwapchainKHR {
    #[hiarc_skip_unsafe]
    pub ash_swapchain: ash::khr::swapchain::Device,
    #[hiarc_skip_unsafe]
    pub swapchain: vk::SwapchainKHR,
}

impl Debug for SwapchainKHR {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SwapchainKHR")
            .field("swapchain", &self.swapchain)
            .finish()
    }
}

impl SwapchainKHR {
    pub fn new(instance: &ash::Instance, device: &ash::Device) -> anyhow::Result<Arc<Self>> {
        let ash_swapchain = ash::khr::swapchain::Device::new(instance, device);
        Ok(Arc::new(Self {
            ash_swapchain,
            swapchain: Default::default(),
        }))
    }

    pub fn new_with_alloc(
        ash_swapchain: ash::khr::swapchain::Device,
        swap_info: vk::SwapchainCreateInfoKHR,
    ) -> anyhow::Result<Arc<Self>, vk::Result> {
        let swapchain = unsafe { ash_swapchain.create_swapchain(&swap_info, None)? };
        Ok(Arc::new(Self {
            ash_swapchain,
            swapchain,
        }))
    }
}

impl Drop for SwapchainKHR {
    fn drop(&mut self) {
        if self.swapchain != vk::SwapchainKHR::null() {
            unsafe {
                self.ash_swapchain.destroy_swapchain(self.swapchain, None);
            }
        }
    }
}
