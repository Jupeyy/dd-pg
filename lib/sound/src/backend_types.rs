use std::time::Duration;

use math::math::vector::vec2;

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

pub struct SoundListenerInfo {
    pub pos: mint::Vector3<f32>,
    pub orientation: mint::Quaternion<f32>,
}

pub trait SoundBackendInterface {
    fn add_or_update_listeners(&mut self, listeners: &[SoundListenerInfo]);
    fn load_sound(&mut self, sound_name: &str, file_data: Vec<u8>);

    fn sound_exists(&mut self, sound: &str) -> bool;
    fn play_at(
        &mut self,
        sound: &str,
        pos: &vec2,
        time_when_started: &Duration,
        cur_time: &Duration,
    );
}
