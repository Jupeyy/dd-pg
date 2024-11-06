use graphics_types::types::GraphicsBackendMemory;
use hiarc::Hiarc;
use sound::sound_mt_types::SoundBackendMemory;

#[derive(Debug, Hiarc)]
pub struct ClientMapImageLoading {
    pub width: u32,
    pub height: u32,
    pub depth: u32,
    pub mem: GraphicsBackendMemory,
    pub name: String,
}

#[derive(Debug, Hiarc)]
pub struct ClientMapSoundLoading {
    pub mem: SoundBackendMemory,
}

#[derive(Debug, Hiarc, Default)]
pub struct ClientMapImagesLoading {
    pub images: Vec<ClientMapImageLoading>,
    pub images_2d_array: Vec<ClientMapImageLoading>,
}

pub type ClientMapSoundsLoading = Vec<ClientMapSoundLoading>;
