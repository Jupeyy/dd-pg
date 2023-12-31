use std::collections::BTreeMap;

use ash::vk;
use graphics_backend_traits::frame_fetcher_plugin::OffscreenCanvasID;
use hashlink::LinkedHashMap;
use hiarc::HiArc;
use hiarc_macro::Hiarc;
use pool::{
    mt_datatypes::{PoolBTreeMap, PoolVec},
    mt_pool::Pool,
};

use super::vulkan_types::RenderPassType;

#[derive(Debug)]
pub struct FrameRenderSubpass {
    /// first generic parameter is the thread index
    pub command_buffers: PoolBTreeMap<usize, vk::CommandBuffer>,
}

impl FrameRenderSubpass {
    pub fn new(command_buffers_pool: &Pool<BTreeMap<usize, vk::CommandBuffer>>) -> Self {
        Self {
            command_buffers: command_buffers_pool.new(),
        }
    }
}

#[derive(Debug)]
pub struct FrameRenderPass {
    pub subpasses: PoolVec<FrameRenderSubpass>,

    pub render_pass_type: RenderPassType,
}

impl FrameRenderPass {
    pub fn new(
        subpasses_pool: &Pool<Vec<FrameRenderSubpass>>,
        render_pass_type: RenderPassType,
    ) -> Self {
        Self {
            subpasses: subpasses_pool.new(),
            render_pass_type,
        }
    }
}

#[derive(Debug)]
pub struct FrameRenderCanvas {
    pub passes: PoolVec<FrameRenderPass>,
}

impl FrameRenderCanvas {
    fn new(passes_pool: &Pool<Vec<FrameRenderPass>>) -> Self {
        Self {
            passes: passes_pool.new(),
        }
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub enum FrameCanvasIndex {
    #[default]
    Onscreen,
    Offscreen(OffscreenCanvasID),
}

/// the render scope consist of the main frame, which is essentially just
/// a command buffer that was started
#[derive(Debug)]
pub struct FrameRenderScope {
    pub main_command_buffer: vk::CommandBuffer,

    // all render passes
    pub onscreen_canvas: FrameRenderCanvas,
    pub offscreen_canvases: LinkedHashMap<OffscreenCanvasID, FrameRenderCanvas>,
}

impl FrameRenderScope {
    pub fn new(passes_pool: &Pool<Vec<FrameRenderPass>>) -> Self {
        Self {
            main_command_buffer: Default::default(),
            onscreen_canvas: FrameRenderCanvas::new(passes_pool),
            offscreen_canvases: Default::default(),
        }
    }

    pub fn clear(&mut self, passes_pool: &Pool<Vec<FrameRenderPass>>) {
        self.main_command_buffer = Default::default();
        self.offscreen_canvases.clear();
        self.onscreen_canvas = FrameRenderCanvas::new(passes_pool);
    }

    pub fn canvas_mode_mut(&mut self, index: FrameCanvasIndex) -> &mut FrameRenderCanvas {
        match index {
            FrameCanvasIndex::Onscreen => &mut self.onscreen_canvas,
            FrameCanvasIndex::Offscreen(id) => self.offscreen_canvases.get_mut(&id).unwrap(),
        }
    }
}

/// a frame of commands
/// the frame is resetted only when swap is called
#[derive(Debug, Hiarc)]
pub struct Frame {
    /// the memory command buffer is always present and always executed before the render commands
    memory_command_buffer: vk::CommandBuffer,

    pub render: FrameRenderScope,

    // pools
    pub command_buffer_exec_pool: Pool<Vec<vk::CommandBuffer>>,
    pub passes_pool: Pool<Vec<FrameRenderPass>>,
    pub subpasses_pool: Pool<Vec<FrameRenderSubpass>>,
    pub command_buffers_pool: Pool<BTreeMap<usize, vk::CommandBuffer>>,
}

impl Frame {
    pub fn new() -> HiArc<parking_lot::Mutex<Self>> {
        let passes_pool = Pool::with_capacity(8);

        HiArc::new(parking_lot::Mutex::new(Self {
            memory_command_buffer: Default::default(),
            render: FrameRenderScope::new(&passes_pool),
            command_buffer_exec_pool: Pool::with_capacity(16),
            passes_pool,
            subpasses_pool: Pool::with_capacity(8),
            command_buffers_pool: Pool::with_capacity(8),
        }))
    }

    pub fn new_frame(&mut self, render_command_buffer: vk::CommandBuffer) {
        self.memory_command_buffer = Default::default();
        self.render.clear(&self.passes_pool);
        self.render.main_command_buffer = render_command_buffer;
    }

    pub fn new_offscreen(&mut self, id: OffscreenCanvasID) {
        self.render
            .offscreen_canvases
            .insert(id, FrameRenderCanvas::new(&self.passes_pool));
    }
}
