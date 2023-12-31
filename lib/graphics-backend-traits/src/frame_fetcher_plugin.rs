use std::fmt::Debug;

use graphics_types::types::ImageFormat;
use pool::mt_datatypes::PoolVec;

pub type OffscreenCanvasID = u64;

#[derive(Debug, Clone, Copy)]
pub enum FetchCanvasIndex {
    Onscreen,
    Offscreen(OffscreenCanvasID),
}

#[derive(Debug)]
pub struct BackendPresentedImageData {
    pub width: u32,
    pub height: u32,
    pub dest_data_buffer: PoolVec<u8>,
    pub img_format: ImageFormat,
}

pub trait BackendFrameFetcher: Debug + Sync + Send + 'static {
    fn next_frame(&self, frame_data: BackendPresentedImageData);

    /// generally a frame fetcher should only fetch the content of a specific canvas
    /// if for whatever reason it changes it can however,
    /// the backend must respect it for every frame.
    fn current_fetch_index(&self) -> FetchCanvasIndex;
}
