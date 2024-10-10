use std::sync::Arc;

use anyhow::anyhow;
use ash::vk;
use hiarc::Hiarc;

use super::logical_device::LogicalDevice;

#[derive(Debug, Hiarc)]
pub struct PipelineLayout {
    #[hiarc_skip_unsafe]
    layout: vk::PipelineLayout,

    device: Arc<LogicalDevice>,
}

impl PipelineLayout {
    pub fn new(
        device: &Arc<LogicalDevice>,
        pipeline_layout_info: &vk::PipelineLayoutCreateInfo,
    ) -> anyhow::Result<Self> {
        let pipe_layout = unsafe {
            device
                .device
                .create_pipeline_layout(pipeline_layout_info, None)
        }
        .map_err(|err| anyhow!("Creating pipeline layout failed: {err}"))?;

        Ok(Self {
            layout: pipe_layout,
            device: device.clone(),
        })
    }

    pub fn layout(&self) -> vk::PipelineLayout {
        self.layout
    }
}

impl Drop for PipelineLayout {
    fn drop(&mut self) {
        unsafe {
            self.device
                .device
                .destroy_pipeline_layout(self.layout, None);
        }
    }
}
