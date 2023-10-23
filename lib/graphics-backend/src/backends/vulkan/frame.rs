use std::collections::BTreeMap;

use ash::vk;

use super::vulkan_types::RenderPassType;

#[derive(Debug)]
pub struct FrameRenderSubpass {
    /// first generic parameter is the thread index
    pub command_buffers: BTreeMap<usize, vk::CommandBuffer>,
}

#[derive(Debug)]
pub struct FrameRenderPass {
    pub subpasses: Vec<FrameRenderSubpass>,

    pub render_pass_type: RenderPassType,
}

/// the render scope consist of the main frame, which is essentially just
/// a command buffer that was started
#[derive(Debug)]
pub struct FrameRenderScope {
    pub main_command_buffer: vk::CommandBuffer,

    // all render passes
    pub passes: Vec<FrameRenderPass>,
}

impl FrameRenderScope {
    pub fn new() -> Self {
        Self {
            main_command_buffer: Default::default(),
            passes: Default::default(),
        }
    }

    pub fn clear(&mut self) {
        self.main_command_buffer = Default::default();
        self.passes.clear()
    }
}

/// a frame of commands
/// the frame is resetted only when swap is called
#[derive(Debug)]
pub struct Frame {
    /// the memory command buffer is always present and always executed before the render commands
    memory_command_buffer: vk::CommandBuffer,

    pub render: FrameRenderScope,
}

impl Frame {
    pub fn new() -> Self {
        Self {
            memory_command_buffer: Default::default(),
            render: FrameRenderScope::new(),
        }
    }

    pub fn clear(&mut self) {
        self.memory_command_buffer = Default::default();
        self.render.clear()
    }
}
