use crate::backend_types::SoundBackendInterface;

pub struct SoundBackendNull {}

impl SoundBackendInterface for SoundBackendNull {
    fn add_listener(&mut self, _pos: mint::Vector3<f32>, _orientation: mint::Quaternion<f32>) {}

    fn load_sound(&mut self, _sound_name: &str, _file_data: Vec<u8>) {}

    fn sound_exists(&mut self, _sound: &str) -> bool {
        false
    }

    fn play_at(
        &mut self,
        _sound: &str,
        _time_when_started: &std::time::Duration,
        _cur_time: &std::time::Duration,
    ) {
    }
}
