use std::{path::Path, rc::Rc};

use graphics_backend::backend::GraphicsBackend;
use graphics_backend_traits::frame_fetcher_plugin::OffscreenCanvasId;
use sound::frame_fetcher_plugin::OffairSoundManagerId;
use sound_backend::sound_backend::SoundBackend;

use crate::types::EncoderSettings;

pub trait AudioVideoEncoder
where
    Self: Sized,
{
    fn new(
        video_frame_buffer_id: OffscreenCanvasId,
        audio_frame_buffer_id: OffairSoundManagerId,
        file_path: &Path,
        backend: &Rc<GraphicsBackend>,
        sound_backend: &Rc<SoundBackend>,
        encoder_settings: EncoderSettings,
    ) -> anyhow::Result<Self>;

    /// The encoder is overloaded with either video or audio frames
    /// and the implementation should skip the next call so the encoders
    /// can catch up.
    fn overloaded(&self) -> bool;
}
