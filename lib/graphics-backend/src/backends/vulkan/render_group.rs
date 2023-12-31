use std::{collections::HashMap, sync::Arc};

use ash::vk;
use graphics_backend_traits::{
    frame_fetcher_plugin::OffscreenCanvasID, plugin::BackendCustomPipeline,
};
use hashlink::LinkedHashMap;
use hiarc::HiArc;
use num_derive::FromPrimitive;

use crate::window::BackendSwapchain;

use super::{
    compiler::compiler::ShaderCompiler, frame_resources::FrameResources,
    logical_device::LogicalDevice, pipeline_cache::PipelineCacheInner, render_pass::CanvasSetup,
    render_setup::RenderSetupCreationType, swapchain::Swapchain, vulkan_allocator::VulkanAllocator,
    vulkan_device::DescriptorLayouts, vulkan_types::DeviceDescriptorPools,
};

#[derive(FromPrimitive, Default, Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u32)]
pub enum StencilOpType {
    #[default]
    None,
    AlwaysPass,
    OnlyWhenPassed,
    OnlyWhenNotPassed,
}
pub const STENCIL_OP_TYPE_COUNT: usize = 4;

#[derive(FromPrimitive, Default, Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u32)]
pub enum ColorMaskType {
    #[default]
    WriteAll,
    WriteColorOnly,
    WriteAlphaOnly,
    WriteNone,
}
pub const COLOR_MASK_TYPE_COUNT: usize = 4;

#[derive(Debug)]
pub struct RenderSetupOptions {
    pub offscreen_extent: vk::Extent2D,
}

#[derive(Debug)]
enum CanvasModeInternal {
    Onscreen,
    Offscreen(OffscreenCanvasID),
}

#[derive(Debug)]
pub enum CanvasMode<'a> {
    Onscreen,
    Offscreen {
        id: OffscreenCanvasID,
        device: &'a HiArc<LogicalDevice>,
        layouts: &'a DescriptorLayouts,
        custom_pipes: &'a Arc<parking_lot::RwLock<Vec<Box<dyn BackendCustomPipeline>>>>,
        pipeline_cache: &'a Option<HiArc<PipelineCacheInner>>,
        standard_texture_descr_pool: &'a HiArc<parking_lot::Mutex<DeviceDescriptorPools>>,
        mem_allocator: &'a HiArc<parking_lot::Mutex<VulkanAllocator>>,
        runtime_threadpool: &'a Arc<rayon::ThreadPool>,
        options: &'a RenderSetupOptions,
        frame_resources: &'a mut FrameResources,
    },
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, Copy)]
pub struct OffscreenCacheProps {
    width: u32,
    height: u32,
}

#[derive(Debug)]
pub struct RenderSetup {
    pub onscreen: HiArc<CanvasSetup>,
    pub offscreens: LinkedHashMap<u64, HiArc<CanvasSetup>>,
    offscreens_cache: HashMap<OffscreenCacheProps, Vec<HiArc<CanvasSetup>>>,

    cur_canvas_mode: CanvasModeInternal,

    pub resources_per_frame: HashMap<u32, FrameResources>,

    // required data
    pub shader_compiler: Arc<ShaderCompiler>,
    _device: HiArc<LogicalDevice>,
}

impl RenderSetup {
    pub fn new(
        device: &HiArc<LogicalDevice>,
        layouts: &DescriptorLayouts,
        custom_pipes: &Arc<parking_lot::RwLock<Vec<Box<dyn BackendCustomPipeline>>>>,
        pipeline_cache: &Option<HiArc<PipelineCacheInner>>,
        standard_texture_descr_pool: &HiArc<parking_lot::Mutex<DeviceDescriptorPools>>,
        mem_allocator: &HiArc<parking_lot::Mutex<VulkanAllocator>>,
        runtime_threadpool: &Arc<rayon::ThreadPool>,
        swapchain: Swapchain,
        swapchain_backend: &BackendSwapchain,
        shader_compiler: ShaderCompiler,
        compile_one_by_one: bool,
    ) -> anyhow::Result<Self> {
        let shader_compiler = Arc::new(shader_compiler);
        let onscreen = CanvasSetup::new(
            device,
            layouts,
            custom_pipes,
            pipeline_cache,
            standard_texture_descr_pool,
            mem_allocator,
            runtime_threadpool,
            &shader_compiler,
            RenderSetupCreationType::Swapchain((swapchain, swapchain_backend)),
            compile_one_by_one,
        )?;

        let res = Self {
            onscreen,
            offscreens: Default::default(),
            offscreens_cache: Default::default(),

            cur_canvas_mode: CanvasModeInternal::Onscreen,

            resources_per_frame: Default::default(),

            shader_compiler,
            _device: device.clone(),
        };
        Ok(res)
    }

    pub fn get(&self) -> &HiArc<CanvasSetup> {
        match self.cur_canvas_mode {
            CanvasModeInternal::Onscreen => &self.onscreen,
            CanvasModeInternal::Offscreen(id) => self.offscreens.get(&id).unwrap(),
        }
    }

    pub fn switch_canvas(&mut self, mode: CanvasMode) -> anyhow::Result<()> {
        self.cur_canvas_mode = match mode {
            CanvasMode::Onscreen => CanvasModeInternal::Onscreen,
            CanvasMode::Offscreen {
                id,
                device,
                layouts,
                custom_pipes,
                pipeline_cache,
                standard_texture_descr_pool,
                mem_allocator,
                runtime_threadpool,
                options,
                frame_resources,
            } => {
                // try to find a old render setup first
                let offscreen_props = OffscreenCacheProps {
                    width: options.offscreen_extent.width,
                    height: options.offscreen_extent.height,
                };
                if let Some(cache) = self.offscreens_cache.get_mut(&offscreen_props) {
                    let render_setup = cache.pop().unwrap();
                    if cache.is_empty() {
                        self.offscreens_cache.remove(&offscreen_props);
                    }
                    self.offscreens.insert(id, render_setup);
                } else {
                    self.offscreens.insert(
                        id,
                        CanvasSetup::new(
                            device,
                            layouts,
                            custom_pipes,
                            pipeline_cache,
                            standard_texture_descr_pool,
                            mem_allocator,
                            runtime_threadpool,
                            &self.shader_compiler,
                            RenderSetupCreationType::Offscreen {
                                extent: options.offscreen_extent,
                                img_count: self.onscreen.swap_chain_image_count(),
                                img_format: self.onscreen.surf_format,
                            },
                            false, // TODO: false or true?
                        )?,
                    );
                }

                frame_resources
                    .render_setups
                    .push(self.offscreens.get(&id).unwrap().inner_arc().clone());

                CanvasModeInternal::Offscreen(id)
            }
        };

        Ok(())
    }

    pub fn new_frame(&mut self) {
        self.offscreens_cache.clear();
        for (_, offscreen) in self.offscreens.drain() {
            let offscreen_props = OffscreenCacheProps {
                width: offscreen.native.swap_img_and_viewport_extent.width,
                height: offscreen.native.swap_img_and_viewport_extent.height,
            };
            if !self.offscreens_cache.contains_key(&offscreen_props) {
                self.offscreens_cache.insert(offscreen_props, Vec::new());
            }
            self.offscreens_cache
                .get_mut(&offscreen_props)
                .unwrap()
                .push(offscreen);
        }
        self.cur_canvas_mode = CanvasModeInternal::Onscreen;
    }
}
