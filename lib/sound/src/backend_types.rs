use std::time::Duration;

use crate::backend::{kiri::kiri::SoundBackendKira, null::null::SoundBackendNull};

pub enum SoundBackend {
    Kira(SoundBackendKira),
    None(SoundBackendNull),
}

impl SoundBackend {
    pub fn as_mut(&mut self) -> &mut dyn SoundBackendInterface {
        match self {
            SoundBackend::Kira(backend) => backend,
            SoundBackend::None(backend) => backend,
        }
    }
}

pub trait SoundBackendInterface {
    fn add_listener(&mut self, pos: mint::Vector3<f32>, orientation: mint::Quaternion<f32>);
    fn load_sound(&mut self, sound_name: &str, file_data: Vec<u8>);

    fn sound_exists(&mut self, sound: &str) -> bool;
    fn play_at(&mut self, sound: &str, time_when_started: &Duration, cur_time: &Duration);
}
