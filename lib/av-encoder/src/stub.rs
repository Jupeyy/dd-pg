use anyhow::anyhow;

use crate::traits::AudioVideoEncoder;

pub struct StubEncoder;

impl AudioVideoEncoder for StubEncoder {
    fn new(
        _video_frame_buffer_id: graphics_backend_traits::frame_fetcher_plugin::OffscreenCanvasId,
        _audio_frame_buffer_id: sound::frame_fetcher_plugin::OffairSoundManagerId,
        _file_path: &std::path::Path,
        _backend: &std::rc::Rc<graphics_backend::backend::GraphicsBackend>,
        _sound_backend: &std::rc::Rc<sound_backend::sound_backend::SoundBackend>,
        _encoder_settings: crate::types::EncoderSettings,
    ) -> anyhow::Result<Self> {
        Err(anyhow!("The stub encoder cannot encode anything."))
    }

    fn overloaded(&self) -> bool {
        false
    }
}
