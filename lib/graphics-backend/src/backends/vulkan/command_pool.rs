use std::cell::RefCell;

use ash::vk;
use hiarc::{HiArc, HiRc};
use hiarc_macro::Hiarc;

use super::{
    command_buffer::CommandBuffers,
    frame::{Frame, FrameCanvasIndex, FrameRenderPass, FrameRenderSubpass},
    frame_resources::RenderThreadFrameResources,
    logical_device::LogicalDevice,
    render_pass::CanvasSetup,
    vulkan_types::RenderPassType,
};

/// automatically ends the command buffer when dropped
/// making it ideal for rendering during a single frame
#[derive(Debug)]
pub struct AutoCommandBuffer {
    pub command_buffer: vk::CommandBuffer,

    device: HiArc<LogicalDevice>,
}

pub enum AutoCommandBufferType<'a> {
    Primary,
    Secondary {
        render: &'a HiArc<CanvasSetup>,

        cur_image_index: u32,

        render_pass_type: RenderPassType,

        render_pass_frame_index: usize,
        buffer_in_order_id: usize,

        canvas_index: FrameCanvasIndex,

        frame: &'a HiArc<parking_lot::Mutex<Frame>>,
    },
}

impl AutoCommandBuffer {
    pub fn new(
        device: HiArc<LogicalDevice>,
        ty: AutoCommandBufferType,
        frame_resources: &mut RenderThreadFrameResources,
        command_buffer: HiRc<CommandBuffers>,
    ) -> anyhow::Result<Self> {
        let mut inheret_info = vk::CommandBufferInheritanceInfo::default();

        let command_buffer = command_buffer.inner_rc().get(frame_resources);

        let mut begin_info = vk::CommandBufferBeginInfo::default();
        match ty {
            AutoCommandBufferType::Primary => {
                begin_info.flags = vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT;
            }
            AutoCommandBufferType::Secondary {
                render,
                cur_image_index,
                render_pass_type,
                render_pass_frame_index,
                buffer_in_order_id,
                canvas_index,
                frame,
            } => {
                begin_info.flags = vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT
                    | vk::CommandBufferUsageFlags::RENDER_PASS_CONTINUE;

                inheret_info.framebuffer = match render_pass_type {
                    RenderPassType::Single => {
                        render.native.framebuffer_list[cur_image_index as usize].buffer
                    }
                    RenderPassType::Switching1 => {
                        render.switching.passes[0].framebuffer_list[cur_image_index as usize].buffer
                    }
                    RenderPassType::Switching2 => {
                        render.switching.passes[1].framebuffer_list[cur_image_index as usize].buffer
                    }
                };
                inheret_info.occlusion_query_enable = vk::FALSE;
                inheret_info.render_pass = match render_pass_type {
                    RenderPassType::Single => render.native.render_pass.pass.pass,
                    RenderPassType::Switching1 => render.switching.passes[0].render_pass.pass.pass,
                    RenderPassType::Switching2 => render.switching.passes[1].render_pass.pass.pass,
                };
                inheret_info.subpass = 0;
                begin_info.p_inheritance_info = &inheret_info;

                let mut frame_g = frame.lock();
                let frame = &mut *frame_g;
                while render_pass_frame_index
                    >= frame.render.canvas_mode_mut(canvas_index).passes.len()
                {
                    frame
                        .render
                        .canvas_mode_mut(canvas_index)
                        .passes
                        .push(FrameRenderPass::new(
                            &frame.subpasses_pool,
                            Default::default(),
                        ));
                }
                frame.render.canvas_mode_mut(canvas_index).passes[render_pass_frame_index]
                    .render_pass_type = render_pass_type;
                if frame.render.canvas_mode_mut(canvas_index).passes[render_pass_frame_index]
                    .subpasses
                    .is_empty()
                {
                    frame.render.canvas_mode_mut(canvas_index).passes[render_pass_frame_index]
                        .subpasses
                        .push(FrameRenderSubpass::new(&frame.command_buffers_pool));
                }

                frame.render.canvas_mode_mut(canvas_index).passes[render_pass_frame_index]
                    .subpasses[0]
                    .command_buffers
                    .insert(buffer_in_order_id, command_buffer);
            }
        }

        unsafe {
            device.device.reset_command_buffer(
                command_buffer,
                vk::CommandBufferResetFlags::RELEASE_RESOURCES,
            )?;
            device
                .device
                .begin_command_buffer(command_buffer, &begin_info)?
        };
        Ok(Self {
            device,
            command_buffer,
        })
    }
}

impl Drop for AutoCommandBuffer {
    fn drop(&mut self) {
        unsafe {
            self.device
                .device
                .end_command_buffer(self.command_buffer)
                .unwrap();
        }
    }
}

#[derive(Debug, Hiarc)]
pub struct CommandPool {
    pub command_pool: vk::CommandPool,
    pub queue_family_index: u32,

    pub primary_command_buffers_in_pool: RefCell<Vec<vk::CommandBuffer>>,
    pub secondary_command_buffers_in_pool: RefCell<Vec<vk::CommandBuffer>>,

    default_primary_buffer_count: usize,
    default_secondary_buffer_count: usize,

    pub device: HiArc<LogicalDevice>,
}

impl CommandPool {
    fn get_command_buffers(
        device: &HiArc<LogicalDevice>,
        command_pool: vk::CommandPool,
        level: vk::CommandBufferLevel,
        count: usize,
    ) -> anyhow::Result<Vec<vk::CommandBuffer>> {
        let mut alloc_info = vk::CommandBufferAllocateInfo::default();
        alloc_info.command_pool = command_pool;
        alloc_info.level = level;
        alloc_info.command_buffer_count = count as u32;

        Ok(unsafe { device.device.allocate_command_buffers(&alloc_info) }?)
    }

    pub fn new(
        device: HiArc<LogicalDevice>,
        queue_family_index: u32,
        default_primary_buffer_count: usize,
        default_secondary_buffer_count: usize,
    ) -> anyhow::Result<HiRc<Self>> {
        let mut create_pool_info = vk::CommandPoolCreateInfo::default();
        create_pool_info.queue_family_index = queue_family_index;
        create_pool_info.flags = vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER;

        let command_pool = unsafe { device.device.create_command_pool(&create_pool_info, None) }?;

        let primary_command_buffers = RefCell::new(if default_primary_buffer_count > 0 {
            Self::get_command_buffers(
                &device,
                command_pool,
                vk::CommandBufferLevel::PRIMARY,
                default_primary_buffer_count,
            )?
        } else {
            Default::default()
        });
        let secondary_command_buffers = RefCell::new(if default_secondary_buffer_count > 0 {
            Self::get_command_buffers(
                &device,
                command_pool,
                vk::CommandBufferLevel::SECONDARY,
                default_secondary_buffer_count,
            )?
        } else {
            Default::default()
        });

        Ok(HiRc::new(Self {
            command_pool,
            queue_family_index,
            device,

            primary_command_buffers_in_pool: primary_command_buffers,
            secondary_command_buffers_in_pool: secondary_command_buffers,

            default_primary_buffer_count,
            default_secondary_buffer_count,
        }))
    }

    pub fn get_render_buffer(
        selfi: &HiRc<Self>,
        ty: AutoCommandBufferType,
        frame_resources: &mut RenderThreadFrameResources,
    ) -> anyhow::Result<AutoCommandBuffer> {
        let pool = match ty {
            AutoCommandBufferType::Primary => &selfi.primary_command_buffers_in_pool,
            AutoCommandBufferType::Secondary { .. } => &selfi.secondary_command_buffers_in_pool,
        };
        let level = match ty {
            AutoCommandBufferType::Primary => vk::CommandBufferLevel::PRIMARY,
            AutoCommandBufferType::Secondary { .. } => vk::CommandBufferLevel::SECONDARY,
        };

        if pool.borrow().is_empty() {
            // allocate new command buffers
            pool.borrow_mut().append(&mut Self::get_command_buffers(
                &selfi.device,
                selfi.command_pool,
                level,
                match ty {
                    AutoCommandBufferType::Primary => selfi.default_primary_buffer_count,
                    AutoCommandBufferType::Secondary { .. } => selfi.default_secondary_buffer_count,
                }
                .max(1),
            )?);
        }

        let item = pool.borrow_mut().pop().unwrap();
        AutoCommandBuffer::new(
            selfi.device.clone(),
            ty,
            frame_resources,
            CommandBuffers::from_pool([item].into(), level, selfi.clone()),
        )
    }
}

impl Drop for CommandPool {
    fn drop(&mut self) {
        let buffers = std::mem::take(&mut *self.primary_command_buffers_in_pool.borrow_mut());
        if !buffers.is_empty() {
            unsafe {
                self.device
                    .device
                    .free_command_buffers(self.command_pool, &buffers);
            }
        }

        let buffers = std::mem::take(&mut *self.secondary_command_buffers_in_pool.borrow_mut());
        // check for empty, vulkan spec doesn't like 0
        if !buffers.is_empty() {
            unsafe {
                self.device
                    .device
                    .free_command_buffers(self.command_pool, &buffers);
            }
        }

        unsafe {
            self.device
                .device
                .destroy_command_pool(self.command_pool, None);
        }
    }
}
