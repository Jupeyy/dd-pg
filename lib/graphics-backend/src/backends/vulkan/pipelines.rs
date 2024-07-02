use std::sync::Arc;

use anyhow::anyhow;
use ash::vk;
use hiarc::Hiarc;

use super::{
    logical_device::LogicalDevice, pipeline_cache::PipelineCacheInner,
    pipeline_layout::PipelineLayout,
};

#[derive(Debug, Hiarc)]
pub struct Pipelines {
    #[hiarc_skip_unsafe]
    pipelines: Vec<vk::Pipeline>,
    pipeline_layouts: Vec<PipelineLayout>,

    device: Arc<LogicalDevice>,
}

impl Pipelines {
    pub fn new(
        device: &Arc<LogicalDevice>,
        cache: &Option<Arc<PipelineCacheInner>>,
        pipeline_infos: Vec<(vk::GraphicsPipelineCreateInfo, PipelineLayout)>,
    ) -> anyhow::Result<Self> {
        let (mut pipeline_infos, pipeline_layouts): (Vec<_>, Vec<_>) =
            pipeline_infos.into_iter().unzip();
        pipeline_infos
            .iter_mut()
            .enumerate()
            .for_each(|(index, info)| info.layout = pipeline_layouts[index].layout());
        let pipelines = unsafe {
            device.device.create_graphics_pipelines(
                cache
                    .as_ref()
                    .map(|cache| cache.cache)
                    .unwrap_or(vk::PipelineCache::null()),
                &pipeline_infos
                    .into_iter()
                    .map(|info| info)
                    .collect::<Vec<_>>(),
                None,
            )
        };
        let pipelines = match pipelines {
            Ok(pipelines) => Ok(pipelines),
            Err((pipelines, res)) => match res {
                vk::Result::PIPELINE_COMPILE_REQUIRED_EXT => Ok(pipelines),
                err => Err(anyhow!("Creating pipelines failed with error code: {err}")),
            },
        }?;

        Ok(Self {
            pipelines,
            pipeline_layouts,
            device: device.clone(),
        })
    }

    /// splits all pipelines into individual objects
    pub fn split_all(mut self) -> Vec<Self> {
        std::mem::take(&mut self.pipelines)
            .into_iter()
            .zip(std::mem::take(&mut self.pipeline_layouts).into_iter())
            .map(|(pipe, layout)| Self {
                pipelines: vec![pipe],
                pipeline_layouts: vec![layout],
                device: self.device.clone(),
            })
            .collect()
    }

    pub fn pipe_and_layout(&self) -> (vk::Pipeline, vk::PipelineLayout) {
        (self.pipelines[0], self.pipeline_layouts[0].layout())
    }
}

impl Drop for Pipelines {
    fn drop(&mut self) {
        unsafe {
            for pipeline in std::mem::take(&mut self.pipelines).into_iter() {
                self.device.device.destroy_pipeline(pipeline, None);
            }
        }
    }
}
