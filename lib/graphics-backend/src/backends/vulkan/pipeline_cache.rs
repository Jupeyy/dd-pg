use ash::vk;
use base_io::{io::IOFileSys, io_batcher::IOBatcherTask};
use base_io_traits::fs_traits::{FileSystemPath, FileSystemType};
use hiarc::{Hi, HiArc};
use hiarc_macro::Hiarc;

use super::logical_device::LogicalDevice;

#[derive(Debug, Hiarc)]
pub struct PipelineCacheInner {
    pub cache: vk::PipelineCache,

    device: HiArc<LogicalDevice>,
}

impl Drop for PipelineCacheInner {
    fn drop(&mut self) {
        unsafe { self.device.device.destroy_pipeline_cache(self.cache, None) };
    }
}

#[derive(Debug, Hiarc)]
pub struct PipelineCache {
    pub(crate) inner: HiArc<PipelineCacheInner>,

    io: IOFileSys,
}

impl PipelineCache {
    pub fn new(
        device: HiArc<LogicalDevice>,
        previous_cache: Option<&Vec<u8>>,
        io: IOFileSys,
    ) -> anyhow::Result<Hi<Self>> {
        let mut create_info = vk::PipelineCacheCreateInfo::builder();
        if let Some(previous_cache) = previous_cache {
            create_info = create_info.initial_data(&previous_cache);
        }

        let cache = unsafe {
            device
                .device
                .create_pipeline_cache(&create_info.build(), None)?
        };

        Ok(Hi::new(Self {
            inner: HiArc::new(PipelineCacheInner { cache, device }),
            io,
        }))
    }

    pub fn load_previous_cache(io: &IOFileSys) -> IOBatcherTask<Option<Vec<u8>>> {
        let fs = io.fs.clone();
        io.io_batcher.spawn(async move {
            let res = fs
                .open_file_in(
                    "cache/vulkan/pipeline.cache",
                    FileSystemPath::OfType(FileSystemType::ReadWrite),
                )
                .await
                .ok();
            Ok(res)
        })
    }
}

impl Drop for PipelineCache {
    fn drop(&mut self) {
        unsafe {
            // fail safe, either it works or it doesn't, no need to handle the error
            if let Ok(cache) = self
                .inner
                .device
                .device
                .get_pipeline_cache_data(self.inner.cache)
            {
                let fs = self.io.fs.clone();
                let _ = self.io.io_batcher.spawn_without_queue(async move {
                    fs.create_dir("cache/vulkan").await?;
                    fs.write_file("cache/vulkan/pipeline.cache", cache).await?;
                    Ok(())
                });
            }
        };
    }
}
