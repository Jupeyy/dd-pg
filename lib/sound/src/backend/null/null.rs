use math::math::vector::vec2;

use crate::backend_types::SoundBackendInterface;

pub struct SoundBackendNull {}

impl SoundBackendInterface for SoundBackendNull {
    fn add_or_update_listeners(&mut self, _listeners: &[crate::backend_types::SoundListenerInfo]) {}

    fn load_sound(&mut self, _sound_name: &str, _file_data: Vec<u8>) {}

    fn sound_exists(&mut self, _sound: &str) -> bool {
        false
    }

    fn play_at(
        &mut self,
        _sound: &str,
        _pos: &vec2,
        _time_when_started: &std::time::Duration,
        _cur_time: &std::time::Duration,
    ) {
    }
}
