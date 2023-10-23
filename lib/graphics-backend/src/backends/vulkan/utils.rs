use std::sync::Arc;

use anyhow::anyhow;
use ash::vk;

use super::{
    barriers::{image_barrier, memory_barrier},
    buffer::Buffer,
    image::{GetImg, Image},
    logical_device::LogicalDevice,
    memory::{SMemoryBlock, SMemoryHeapQueueElement},
    memory_block::SDeviceMemoryBlock,
    vulkan_allocator::{
        STAGING_BUFFER_CACHE_ID, STAGING_BUFFER_IMAGE_CACHE_ID, VERTEX_BUFFER_CACHE_ID,
    },
    vulkan_limits::Limits,
    vulkan_mem::{BufferAllocationError, ImageAllocationError},
};

pub fn copy_buffer_to_image(
    device: &Arc<LogicalDevice>,
    command_buffer: vk::CommandBuffer,
    buffer: &Arc<Buffer>,
    buffer_offset: vk::DeviceSize,
    image: &Arc<Image>,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    depth: usize,
) -> anyhow::Result<()> {
    let mut region = vk::BufferImageCopy::default();
    region.buffer_offset = buffer_offset;
    region.buffer_row_length = 0;
    region.buffer_image_height = 0;
    region.image_subresource.aspect_mask = vk::ImageAspectFlags::COLOR;
    region.image_subresource.mip_level = 0;
    region.image_subresource.base_array_layer = 0;
    region.image_subresource.layer_count = depth as u32;
    region.image_offset = vk::Offset3D { x, y, z: 0 };
    region.image_extent = vk::Extent3D {
        width,
        height,
        depth: 1,
    };

    unsafe {
        device.device.cmd_copy_buffer_to_image(
            command_buffer,
            buffer.buffer,
            image.image,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            &[region],
        );
    }

    Ok(())
}

pub fn build_mipmaps(
    device: &Arc<LogicalDevice>,
    command_buffer: vk::CommandBuffer,
    image: &Arc<Image>,
    _image_format: vk::Format,
    width: usize,
    height: usize,
    depth: usize,
    mip_map_level_count: usize,
) -> anyhow::Result<()> {
    let mut barrier = vk::ImageMemoryBarrier::default();
    barrier.image = image.image;
    barrier.src_queue_family_index = vk::QUEUE_FAMILY_IGNORED;
    barrier.dst_queue_family_index = vk::QUEUE_FAMILY_IGNORED;
    barrier.subresource_range.aspect_mask = vk::ImageAspectFlags::COLOR;
    barrier.subresource_range.level_count = 1;
    barrier.subresource_range.base_array_layer = 0;
    barrier.subresource_range.layer_count = depth as u32;

    let mut tmp_mip_width: i32 = width as i32;
    let mut tmp_mip_height: i32 = height as i32;

    for i in 1..mip_map_level_count {
        barrier.subresource_range.base_mip_level = (i - 1) as u32;
        barrier.old_layout = vk::ImageLayout::TRANSFER_DST_OPTIMAL;
        barrier.new_layout = vk::ImageLayout::TRANSFER_SRC_OPTIMAL;
        barrier.src_access_mask = vk::AccessFlags::TRANSFER_WRITE;
        barrier.dst_access_mask = vk::AccessFlags::TRANSFER_READ;

        unsafe {
            device.device.cmd_pipeline_barrier(
                command_buffer,
                vk::PipelineStageFlags::TRANSFER,
                vk::PipelineStageFlags::TRANSFER,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                &[barrier],
            );
        }

        let mut blit = vk::ImageBlit::default();
        blit.src_offsets[0] = vk::Offset3D::default();
        blit.src_offsets[1] = vk::Offset3D {
            x: tmp_mip_width,
            y: tmp_mip_height,
            z: 1,
        };
        blit.src_subresource.aspect_mask = vk::ImageAspectFlags::COLOR;
        blit.src_subresource.mip_level = (i - 1) as u32;
        blit.src_subresource.base_array_layer = 0;
        blit.src_subresource.layer_count = depth as u32;
        blit.dst_offsets[0] = vk::Offset3D::default();
        blit.dst_offsets[1] = vk::Offset3D {
            x: if tmp_mip_width > 1 {
                tmp_mip_width / 2
            } else {
                1
            },
            y: if tmp_mip_height > 1 {
                tmp_mip_height / 2
            } else {
                1
            },
            z: 1,
        };
        blit.dst_subresource.aspect_mask = vk::ImageAspectFlags::COLOR;
        blit.dst_subresource.mip_level = i as u32;
        blit.dst_subresource.base_array_layer = 0;
        blit.dst_subresource.layer_count = depth as u32;

        unsafe {
            device.device.cmd_blit_image(
                command_buffer,
                image.image,
                vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                image.image,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                &[blit],
                if device
                    .phy_device
                    .config
                    .read()
                    .unwrap()
                    .allows_linear_blitting
                {
                    vk::Filter::LINEAR
                } else {
                    vk::Filter::NEAREST
                },
            );
        }

        barrier.old_layout = vk::ImageLayout::TRANSFER_SRC_OPTIMAL;
        barrier.new_layout = vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL;
        barrier.src_access_mask = vk::AccessFlags::TRANSFER_READ;
        barrier.dst_access_mask = vk::AccessFlags::SHADER_READ;

        unsafe {
            device.device.cmd_pipeline_barrier(
                command_buffer,
                vk::PipelineStageFlags::TRANSFER,
                vk::PipelineStageFlags::FRAGMENT_SHADER,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                &[barrier],
            );
        }

        if tmp_mip_width > 1 {
            tmp_mip_width /= 2;
        }
        if tmp_mip_height > 1 {
            tmp_mip_height /= 2;
        }
    }

    barrier.subresource_range.base_mip_level = (mip_map_level_count - 1) as u32;
    barrier.old_layout = vk::ImageLayout::TRANSFER_DST_OPTIMAL;
    barrier.new_layout = vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL;
    barrier.src_access_mask = vk::AccessFlags::TRANSFER_WRITE;
    barrier.dst_access_mask = vk::AccessFlags::SHADER_READ;

    unsafe {
        device.device.cmd_pipeline_barrier(
            command_buffer,
            vk::PipelineStageFlags::TRANSFER,
            vk::PipelineStageFlags::FRAGMENT_SHADER,
            vk::DependencyFlags::empty(),
            &[],
            &[],
            &[barrier],
        );
    }

    Ok(())
}

pub fn complete_texture(
    device: &Arc<LogicalDevice>,
    command_buffer: vk::CommandBuffer,
    staging_buffer: &SMemoryBlock<STAGING_BUFFER_IMAGE_CACHE_ID>,
    img: &Arc<Image>,
    format: vk::Format,
    width: usize,
    height: usize,
    depth: usize,
    _pixel_size: usize,
    mip_map_level_count: usize,
) -> anyhow::Result<(), ImageAllocationError> {
    let img_format = format;

    image_barrier(
        device,
        command_buffer,
        img.as_ref(),
        0,
        mip_map_level_count,
        0,
        depth,
        vk::ImageLayout::UNDEFINED,
        vk::ImageLayout::TRANSFER_DST_OPTIMAL,
    )
    .map_err(|_| ImageAllocationError::MemoryRelatedOperationFailed)?;
    copy_buffer_to_image(
        device,
        command_buffer,
        staging_buffer.buffer.as_ref().unwrap(),
        staging_buffer.heap_data.offset_to_align as u64,
        img,
        0,
        0,
        width as u32,
        height as u32,
        depth,
    )
    .map_err(|_| ImageAllocationError::MemoryRelatedOperationFailed)?;

    if mip_map_level_count > 1 {
        build_mipmaps(
            device,
            command_buffer,
            img,
            img_format,
            width,
            height,
            depth,
            mip_map_level_count,
        )
        .map_err(|_| ImageAllocationError::MemoryRelatedOperationFailed)?;
    } else {
        image_barrier(
            device,
            command_buffer,
            img.as_ref(),
            0,
            1,
            0,
            depth,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
        )
        .map_err(|_| ImageAllocationError::MemoryRelatedOperationFailed)?;
    }

    Ok(())
}

pub fn copy_buffer(
    device: &Arc<LogicalDevice>,
    command_buffer: vk::CommandBuffer,
    src_buffer: &Arc<Buffer>,
    dst_buffer: &Arc<Buffer>,
    src_offset: vk::DeviceSize,
    dst_offset: vk::DeviceSize,
    copy_size: vk::DeviceSize,
) -> anyhow::Result<()> {
    let mut copy_region = vk::BufferCopy::default();
    copy_region.src_offset = src_offset;
    copy_region.dst_offset = dst_offset;
    copy_region.size = copy_size;
    unsafe {
        device.device.cmd_copy_buffer(
            command_buffer,
            src_buffer.buffer,
            dst_buffer.buffer,
            &[copy_region],
        );
    }

    Ok(())
}

pub fn complete_buffer_object(
    device: &Arc<LogicalDevice>,
    command_buffer: vk::CommandBuffer,
    staging_buffer: &SMemoryBlock<STAGING_BUFFER_CACHE_ID>,
    buffer_mem: &SMemoryBlock<VERTEX_BUFFER_CACHE_ID>,
    buffer_data_size: vk::DeviceSize,
) -> anyhow::Result<(), BufferAllocationError> {
    let vertex_buffer = buffer_mem.buffer.clone().unwrap();
    let buffer_offset = buffer_mem.heap_data.offset_to_align;

    memory_barrier(
        device,
        command_buffer,
        &vertex_buffer,
        buffer_offset as u64,
        buffer_data_size,
        vk::AccessFlags::VERTEX_ATTRIBUTE_READ,
        true,
    )
    .map_err(|_| BufferAllocationError::MemoryRelatedOperationFailed)?;
    copy_buffer(
        device,
        command_buffer,
        &staging_buffer.buffer.as_ref().unwrap(),
        &vertex_buffer,
        staging_buffer.heap_data.offset_to_align as u64,
        buffer_offset as u64,
        buffer_data_size,
    )
    .map_err(|_| BufferAllocationError::MemoryRelatedOperationFailed)?;
    memory_barrier(
        device,
        command_buffer,
        &vertex_buffer,
        buffer_offset as u64,
        buffer_data_size,
        vk::AccessFlags::VERTEX_ATTRIBUTE_READ,
        false,
    )
    .map_err(|_| BufferAllocationError::MemoryRelatedOperationFailed)?;

    Ok(())
}

pub fn get_memory_range(
    buffer_mem: &SDeviceMemoryBlock,
    heap_data: &SMemoryHeapQueueElement,
    limits: &Limits,
) -> vk::MappedMemoryRange {
    let mut upload_range = vk::MappedMemoryRange::default();
    upload_range.memory = buffer_mem.mem;
    upload_range.offset = heap_data.offset_to_align as u64;
    let alignment_mod =
        heap_data.allocation_size as vk::DeviceSize % limits.non_coherent_mem_alignment;
    let mut alignment_req = limits.non_coherent_mem_alignment - alignment_mod;
    if alignment_mod == 0 {
        alignment_req = 0;
    }
    upload_range.size = heap_data.allocation_size as u64 + alignment_req;

    if upload_range.offset + upload_range.size > buffer_mem.size {
        upload_range.size = vk::WHOLE_SIZE;
    }
    upload_range
}

pub fn copy_color_attachment_to_present_src(
    device: &Arc<LogicalDevice>,
    command_buffer: vk::CommandBuffer,
    img_color_attachment: &dyn GetImg,
    img_present: &dyn GetImg,
    width: u32,
    height: u32,
) -> anyhow::Result<()> {
    // transition the current frame image to shader_read
    image_barrier(
        device,
        command_buffer,
        img_present,
        0,
        1,
        0,
        1,
        vk::ImageLayout::PRESENT_SRC_KHR,
        vk::ImageLayout::TRANSFER_DST_OPTIMAL,
    )
    .map_err(|_| anyhow!("could not transition image for swapping framebuffer"))?;
    image_barrier(
        device,
        command_buffer,
        img_color_attachment,
        0,
        1,
        0,
        1,
        vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
        vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
    )
    .map_err(|_| anyhow!("could not transition image for swapping framebuffer"))?;

    // Otherwise use image copy (requires us to manually flip components)
    let mut image_copy_region = vk::ImageCopy::default();
    image_copy_region.src_subresource.aspect_mask = vk::ImageAspectFlags::COLOR;
    image_copy_region.src_subresource.layer_count = 1;
    image_copy_region.dst_subresource.aspect_mask = vk::ImageAspectFlags::COLOR;
    image_copy_region.dst_subresource.layer_count = 1;
    image_copy_region.extent.width = width;
    image_copy_region.extent.height = height;
    image_copy_region.extent.depth = 1;

    // Issue the copy command
    unsafe {
        device.device.cmd_copy_image(
            command_buffer,
            img_color_attachment.img(),
            vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
            img_present.img(),
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            &[image_copy_region],
        );
    }

    // transition the current frame image to shader_read
    image_barrier(
        device,
        command_buffer,
        img_present,
        0,
        1,
        0,
        1,
        vk::ImageLayout::TRANSFER_DST_OPTIMAL,
        vk::ImageLayout::PRESENT_SRC_KHR,
    )
    .map_err(|_| anyhow!("could not transition image for swapping framebuffer"))?;
    image_barrier(
        device,
        command_buffer,
        img_color_attachment,
        0,
        1,
        0,
        1,
        vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
        vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
    )
    .map_err(|_| anyhow!("could not transition image for swapping framebuffer"))?;

    Ok(())
}
