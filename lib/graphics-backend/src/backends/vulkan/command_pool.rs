use std::{cell::RefCell, rc::Rc, sync::Arc};

use ash::vk;

use super::{
    frame::{Frame, FrameRenderPass, FrameRenderSubpass},
    logical_device::LogicalDevice,
    render_pass::RenderSetupGroup,
    vulkan_types::RenderPassType,
};

/// automatically ends the command buffer when dropped
/// making it ideal for rendering during a single frame
#[derive(Debug)]
pub struct AutoCommandBuffer {
    pub command_buffer: vk::CommandBuffer,

    is_primary: bool,

    pool_cleanup: Rc<RefCell<CommandPoolCleanups>>,

    device: Arc<LogicalDevice>,
}

pub enum AutoCommandBufferType<'a> {
    Primary,
    Secondary {
        render: &'a RenderSetupGroup,

        cur_image_index: u32,

        render_pass_type: RenderPassType,

        render_pass_frame_index: usize,
        buffer_in_order_id: usize,
    },
}

impl AutoCommandBuffer {
    pub fn new(
        device: Arc<LogicalDevice>,
        pool_cleanup: Rc<RefCell<CommandPoolCleanups>>,
        frame: &Arc<spin::Mutex<Frame>>,
        ty: AutoCommandBufferType,
        command_buffer: vk::CommandBuffer,
    ) -> anyhow::Result<Self> {
        let mut inheret_info = vk::CommandBufferInheritanceInfo::default();

        let mut begin_info = vk::CommandBufferBeginInfo::default();
        let is_primary = match ty {
            AutoCommandBufferType::Primary => {
                begin_info.flags = vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT;
                true
            }
            AutoCommandBufferType::Secondary {
                render,
                cur_image_index,
                render_pass_type,
                render_pass_frame_index,
                buffer_in_order_id,
            } => {
                begin_info.flags = vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT
                    | vk::CommandBufferUsageFlags::RENDER_PASS_CONTINUE;

                inheret_info.framebuffer = match render_pass_type {
                    RenderPassType::Single => {
                        render.get().native.framebuffer_list[cur_image_index as usize]
                    }
                    RenderPassType::Switching1 => {
                        render.get().switching.passes[0].framebuffer_list[cur_image_index as usize]
                    }
                    RenderPassType::Switching2 => {
                        render.get().switching.passes[1].framebuffer_list[cur_image_index as usize]
                    }
                };
                inheret_info.occlusion_query_enable = vk::FALSE;
                inheret_info.render_pass = match render_pass_type {
                    RenderPassType::Single => render.get().native.render_pass.pass,
                    RenderPassType::Switching1 => render.get().switching.passes[0].render_pass.pass,
                    RenderPassType::Switching2 => render.get().switching.passes[1].render_pass.pass,
                };
                inheret_info.subpass = 0;
                begin_info.p_inheritance_info = &inheret_info;

                let mut frame = frame.lock();
                while render_pass_frame_index >= frame.render.passes.len() {
                    frame.render.passes.push(FrameRenderPass {
                        subpasses: Default::default(),
                        render_pass_type: Default::default(),
                    });
                }
                frame.render.passes[render_pass_frame_index].render_pass_type = render_pass_type;
                if frame.render.passes[render_pass_frame_index]
                    .subpasses
                    .is_empty()
                {
                    frame.render.passes[render_pass_frame_index].subpasses.push(
                        FrameRenderSubpass {
                            command_buffers: Default::default(),
                        },
                    );
                }

                frame.render.passes[render_pass_frame_index].subpasses[0]
                    .command_buffers
                    .insert(buffer_in_order_id, command_buffer);
                false
            }
        };

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
            pool_cleanup,
            is_primary,
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

        let mut cleanup = self.pool_cleanup.borrow_mut();
        let index = cleanup.cur_frame_index;
        let cleanup_of_frame = &mut cleanup.cleanups[index];
        let buffers = if self.is_primary {
            &mut cleanup_of_frame.primary_command_buffers
        } else {
            &mut cleanup_of_frame.secondary_command_buffers
        };
        buffers.push(self.command_buffer);
    }
}

#[derive(Debug, Default)]
pub struct CommandPoolCleanup {
    primary_command_buffers: Vec<vk::CommandBuffer>,
    secondary_command_buffers: Vec<vk::CommandBuffer>,
}

#[derive(Debug, Default)]
pub struct CommandPoolCleanups {
    cleanups: Vec<CommandPoolCleanup>,

    cur_frame_index: usize,
}

#[derive(Debug)]
pub struct CommandPool {
    pub command_pool: vk::CommandPool,
    pub queue_family_index: u32,

    primary_command_buffers_in_pool: RefCell<Vec<vk::CommandBuffer>>,
    secondary_command_buffers_in_pool: RefCell<Vec<vk::CommandBuffer>>,

    cleanup_per_frame: Rc<RefCell<CommandPoolCleanups>>,

    default_primary_buffer_count: usize,
    default_secondary_buffer_count: usize,

    pub device: Arc<LogicalDevice>,
}

impl CommandPool {
    fn get_command_buffers(
        device: &Arc<LogicalDevice>,
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
        device: Arc<LogicalDevice>,
        queue_family_index: u32,
        default_primary_buffer_count: usize,
        default_secondary_buffer_count: usize,
    ) -> anyhow::Result<Rc<Self>> {
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

        Ok(Rc::new(Self {
            command_pool,
            queue_family_index,
            device,

            primary_command_buffers_in_pool: primary_command_buffers,
            secondary_command_buffers_in_pool: secondary_command_buffers,

            cleanup_per_frame: Default::default(),

            default_primary_buffer_count,
            default_secondary_buffer_count,
        }))
    }

    pub fn set_frame_count(&self, frame_count: usize) {
        let mut cleanup = self.cleanup_per_frame.borrow_mut();
        for mut cleanups in cleanup.cleanups.drain(..) {
            self.primary_command_buffers_in_pool
                .borrow_mut()
                .append(&mut cleanups.primary_command_buffers);
            self.secondary_command_buffers_in_pool
                .borrow_mut()
                .append(&mut cleanups.secondary_command_buffers);
        }
        cleanup.cur_frame_index = 0;
        cleanup
            .cleanups
            .resize_with(frame_count, || Default::default());
    }

    pub fn set_frame_index(&self, frame_index: usize) {
        self.cleanup_per_frame.borrow_mut().cur_frame_index = frame_index;

        self.clear_frame(frame_index);
    }

    fn clear_frame(&self, frame_index: usize) {
        let mut cleanup = self.cleanup_per_frame.borrow_mut();
        let cleanups = &mut cleanup.cleanups[frame_index];

        self.primary_command_buffers_in_pool
            .borrow_mut()
            .append(&mut cleanups.primary_command_buffers);
        self.secondary_command_buffers_in_pool
            .borrow_mut()
            .append(&mut cleanups.secondary_command_buffers);
    }

    pub fn get_render_buffer(
        &self,
        ty: AutoCommandBufferType,
        frame: &Arc<spin::Mutex<Frame>>,
    ) -> anyhow::Result<AutoCommandBuffer> {
        let pool = match ty {
            AutoCommandBufferType::Primary => &self.primary_command_buffers_in_pool,
            AutoCommandBufferType::Secondary { .. } => &self.secondary_command_buffers_in_pool,
        };

        if pool.borrow().is_empty() {
            // allocate new command buffers
            pool.borrow_mut().append(&mut Self::get_command_buffers(
                &self.device,
                self.command_pool,
                match ty {
                    AutoCommandBufferType::Primary => vk::CommandBufferLevel::PRIMARY,
                    AutoCommandBufferType::Secondary { .. } => vk::CommandBufferLevel::SECONDARY,
                },
                match ty {
                    AutoCommandBufferType::Primary => self.default_primary_buffer_count,
                    AutoCommandBufferType::Secondary { .. } => self.default_secondary_buffer_count,
                }
                .max(1),
            )?);
        }

        let item = pool.borrow_mut().pop().unwrap();
        AutoCommandBuffer::new(
            self.device.clone(),
            self.cleanup_per_frame.clone(),
            frame,
            ty,
            item,
        )
    }
}

impl Drop for CommandPool {
    fn drop(&mut self) {
        let frame_count = self.cleanup_per_frame.borrow().cleanups.len();
        for i in 0..frame_count {
            self.clear_frame(i);
        }

        let buffers = std::mem::take(&mut *self.primary_command_buffers_in_pool.borrow_mut());
        if !buffers.is_empty() {
            self.device
                .memory_allocator
                .lock()
                .free_command_buffers(self.command_pool, buffers);
        }

        let buffers = std::mem::take(&mut *self.secondary_command_buffers_in_pool.borrow_mut());
        // check for empty, vulkan spec doesn't like 0
        if !buffers.is_empty() {
            self.device
                .memory_allocator
                .lock()
                .free_command_buffers(self.command_pool, buffers);
        }

        self.device
            .memory_allocator
            .lock()
            .free_command_pool(self.command_pool);
    }
}
