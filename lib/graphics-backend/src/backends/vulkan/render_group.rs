use std::{
    collections::HashMap,
    sync::{atomic::AtomicUsize, Arc},
};

use anyhow::anyhow;
use ash::vk;
use graphics_backend_traits::frame_fetcher_plugin::OffscreenCanvasID;
use hashlink::LinkedHashMap;
use hiarc::Hiarc;
use num_derive::FromPrimitive;

use crate::{backend::CustomPipelines, window::BackendSwapchain};

use super::{
    compiler::compiler::ShaderCompiler, frame_resources::FrameResources,
    logical_device::LogicalDevice, pipeline_cache::PipelineCacheInner, render_pass::CanvasSetup,
    render_setup::CanvasSetupCreationType, swapchain::Swapchain, vulkan_allocator::VulkanAllocator,
    vulkan_device::DescriptorLayouts, vulkan_types::DeviceDescriptorPools,
};

#[repr(u32)]
#[derive(FromPrimitive, Hiarc, Default, Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum StencilOpType {
    #[default]
    None,
    AlwaysPass,
    OnlyWhenPassed,
    OnlyWhenNotPassed,
}
pub const STENCIL_OP_TYPE_COUNT: usize = 4;

#[derive(FromPrimitive, Hiarc, Default, Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u32)]
pub enum ColorWriteMaskType {
    #[default]
    All,
    ColorOnly,
    AlphaOnly,
    None,
}
pub const COLOR_MASK_TYPE_COUNT: usize = 4;

#[derive(Debug)]
pub struct RenderSetupOptions {
    pub offscreen_extent: vk::Extent2D,
}

#[derive(Debug, Hiarc)]
enum CanvasModeInternal {
    Onscreen,
    Offscreen(OffscreenCanvasID),
}

#[derive(Debug)]
pub enum CanvasMode<'a> {
    Onscreen,
    Offscreen {
        id: OffscreenCanvasID,
        device: &'a Arc<LogicalDevice>,
        layouts: &'a DescriptorLayouts,
        custom_pipes: &'a CustomPipelines,
        pipeline_cache: &'a Option<Arc<PipelineCacheInner>>,
        standard_texture_descr_pool: &'a Arc<parking_lot::Mutex<DeviceDescriptorPools>>,
        mem_allocator: &'a Arc<parking_lot::Mutex<VulkanAllocator>>,
        runtime_threadpool: &'a Arc<rayon::ThreadPool>,
        options: &'a RenderSetupOptions,
        frame_resources: &'a mut FrameResources,
        has_multi_sampling: Option<u32>,
    },
}

#[derive(Debug, Hiarc, Hash, PartialEq, Eq, Clone, Copy)]
pub struct OffscreenCacheProps {
    width: u32,
    height: u32,
}

#[derive(Debug, Hiarc)]
pub struct RenderSetup {
    pub onscreen: Arc<CanvasSetup>,
    pub offscreens: LinkedHashMap<u64, Arc<CanvasSetup>>,
    offscreens_cache: HashMap<OffscreenCacheProps, Vec<Arc<CanvasSetup>>>,

    cur_canvas_mode: CanvasModeInternal,

    pub resources_per_frame: HashMap<u32, FrameResources>,

    // required data
    pub shader_compiler: Arc<ShaderCompiler>,
    pub pipeline_compile_in_queue: Arc<AtomicUsize>,
    _device: Arc<LogicalDevice>,
}

impl RenderSetup {
    pub fn new(
        device: &Arc<LogicalDevice>,
        layouts: &DescriptorLayouts,
        custom_pipes: &CustomPipelines,
        pipeline_cache: &Option<Arc<PipelineCacheInner>>,
        standard_texture_descr_pool: &Arc<parking_lot::Mutex<DeviceDescriptorPools>>,
        mem_allocator: &Arc<parking_lot::Mutex<VulkanAllocator>>,
        runtime_threadpool: &Arc<rayon::ThreadPool>,
        swapchain: Swapchain,
        swapchain_backend: &BackendSwapchain,
        shader_compiler: ShaderCompiler,
        compile_one_by_one: bool,
        has_multi_sampling: Option<u32>,
    ) -> anyhow::Result<Self> {
        let pipeline_compile_in_queue: Arc<AtomicUsize> = Default::default();

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
            CanvasSetupCreationType::Swapchain((swapchain, swapchain_backend)),
            if compile_one_by_one {
                Some(&pipeline_compile_in_queue)
            } else {
                None
            },
            has_multi_sampling,
        )?;

        let res = Self {
            onscreen,
            offscreens: Default::default(),
            offscreens_cache: Default::default(),

            cur_canvas_mode: CanvasModeInternal::Onscreen,

            resources_per_frame: Default::default(),

            shader_compiler,
            pipeline_compile_in_queue,
            _device: device.clone(),
        };
        Ok(res)
    }

    pub fn get(&self) -> &Arc<CanvasSetup> {
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
                has_multi_sampling,
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
                            CanvasSetupCreationType::Offscreen {
                                extent: options.offscreen_extent,
                                img_count: self.onscreen.swap_chain_image_count(),
                                img_format: self.onscreen.surf_format,
                            },
                            Some(&self.pipeline_compile_in_queue),
                            has_multi_sampling,
                        )?,
                    );
                }

                frame_resources
                    .render_setups
                    .push(self.offscreens.get(&id).unwrap().clone());

                CanvasModeInternal::Offscreen(id)
            }
        };

        Ok(())
    }

    pub fn new_frame(&mut self, frame_resources: &mut FrameResources) -> anyhow::Result<()> {
        self.offscreens_cache.clear();
        for (_, offscreen) in self.offscreens.drain() {
            let offscreen_props = OffscreenCacheProps {
                width: offscreen.native.swap_img_and_viewport_extent.width,
                height: offscreen.native.swap_img_and_viewport_extent.height,
            };
            self.offscreens_cache.entry(offscreen_props).or_default();
            self.offscreens_cache
                .get_mut(&offscreen_props)
                .unwrap()
                .push(offscreen);
        }
        self.cur_canvas_mode = CanvasModeInternal::Onscreen;

        self.try_finish_compile(frame_resources)
    }

    pub fn try_finish_compile(
        &mut self,
        frame_resources: &mut FrameResources,
    ) -> anyhow::Result<()> {
        if self
            .pipeline_compile_in_queue
            .load(std::sync::atomic::Ordering::SeqCst)
            > 0
        {
            Arc::get_mut(&mut self.onscreen)
                .ok_or(anyhow!(
                    "could not get onscreen canvas setup as mut form Arc"
                ))?
                .try_finish_compile(frame_resources)?;
            for offscreen in self.offscreens.values_mut() {
                Arc::get_mut(offscreen)
                    .ok_or(anyhow!(
                        "could not get offscreen canvas setup as mut form Arc"
                    ))?
                    .try_finish_compile(frame_resources)?;
            }
        }

        Ok(())
    }
}
