use std::time::Duration;

use hashlink::LinkedHashMap;
use kira::{
    clock::{ClockHandle, ClockSpeed},
    manager::{backend::DefaultBackend, AudioManager, AudioManagerSettings},
    sound::static_sound::{StaticSoundData, StaticSoundHandle, StaticSoundSettings},
    spatial::{
        listener::ListenerSettings,
        scene::{SpatialSceneHandle, SpatialSceneSettings},
    },
    OutputDestination,
};

use crate::backend_types::SoundBackendInterface;

pub struct Sound {
    data: StaticSoundData,
    handle: Option<StaticSoundHandle>,
    _start_time: Duration,
}

pub struct SoundBackendKira {
    manager: AudioManager<DefaultBackend>,

    clock_handle: ClockHandle,
    scene: SpatialSceneHandle,

    files: LinkedHashMap<String, Sound>,
}

impl SoundBackendKira {
    pub fn new() -> anyhow::Result<Self> {
        let mut manager = AudioManager::<DefaultBackend>::new(AudioManagerSettings::default())?;
        let clock_handle = manager.add_clock(ClockSpeed::TicksPerMinute(1000.0 * 60.0))?;
        let scene = manager.add_spatial_scene(SpatialSceneSettings::new())?;
        Ok(Self {
            manager,
            clock_handle,
            scene,

            files: Default::default(),
        })
    }
}

impl SoundBackendInterface for SoundBackendKira {
    fn add_listener(&mut self, pos: mint::Vector3<f32>, orientation: mint::Quaternion<f32>) {
        self // TODO:
            .scene
            .add_listener(pos, orientation, ListenerSettings::new())
            .unwrap();
    }

    fn load_sound(&mut self, sound_name: &str, file_data: Vec<u8>) {
        let sound_data = StaticSoundData::from_cursor(
            std::io::Cursor::new(file_data),
            StaticSoundSettings::default(),
        )
        .unwrap();
        self.files.insert(
            sound_name.to_string(),
            Sound {
                data: sound_data,
                handle: None,
                _start_time: Duration::ZERO,
            },
        );
    }

    fn sound_exists(&mut self, sound: &str) -> bool {
        self.files.contains_key(sound)
    }

    fn play_at(&mut self, sound: &str, time_when_started: &Duration, cur_time: &Duration) {
        let sound = self.files.get_mut(sound).unwrap();
        let sound_data = sound.data.with_settings(
            StaticSoundSettings::new()
                .start_time(
                    self.clock_handle.time() + (*cur_time - *time_when_started).as_millis() as u64,
                )
                .output_destination(OutputDestination::MAIN_TRACK),
        );
        let play = self.manager.play(sound_data).unwrap();
        sound.handle = Some(play);
    }
}
