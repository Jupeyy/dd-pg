use std::time::Duration;

use hashlink::LinkedHashMap;
use kira::{
    clock::{ClockHandle, ClockSpeed},
    manager::{backend::DefaultBackend, AudioManager, AudioManagerSettings},
    sound::static_sound::{StaticSoundData, StaticSoundHandle, StaticSoundSettings},
    spatial::{
        emitter::{EmitterHandle, EmitterSettings},
        listener::{ListenerHandle, ListenerSettings},
        scene::{SpatialSceneHandle, SpatialSceneSettings},
    },
    OutputDestination,
};
use math::math::vector::vec2;
use mint::Vector3;

use crate::backend_types::{SoundBackendInterface, SoundListenerInfo};

pub struct Sound {
    data: StaticSoundData,
    handle: Option<(StaticSoundHandle, EmitterHandle)>,
    _start_time: Duration,
}

pub struct SoundBackendKira {
    manager: AudioManager<DefaultBackend>,

    clock_handle: ClockHandle,
    scene: SpatialSceneHandle,

    active_listeners: Vec<ListenerHandle>,

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

            active_listeners: Default::default(),

            files: Default::default(),
        })
    }
}

impl SoundBackendInterface for SoundBackendKira {
    fn add_or_update_listeners(&mut self, listeners: &[SoundListenerInfo]) {
        self.active_listeners.resize_with(listeners.len(), || {
            self // TODO:
                .scene
                .add_listener(
                    mint::Vector3 {
                        x: 0.0,
                        y: 0.0,
                        z: 0.0,
                    },
                    mint::Quaternion {
                        s: 1.0,
                        v: mint::Vector3 {
                            x: 0.0,
                            y: 0.0,
                            z: 0.0,
                        },
                    },
                    ListenerSettings::new(),
                )
                .unwrap()
        });
        for (index, active_listener) in self.active_listeners.iter_mut().enumerate() {
            // TODO: tween?, unwrap
            active_listener
                .set_position(listeners[index].pos, Default::default())
                .unwrap();
            // TODO: tween?, unwrap
            active_listener
                .set_orientation(listeners[index].orientation, Default::default())
                .unwrap();
        }
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

    fn play_at(
        &mut self,
        sound: &str,
        pos: &vec2,
        time_when_started: &Duration,
        cur_time: &Duration,
    ) {
        let sound = self.files.get_mut(sound).unwrap();

        let emitter = self
            .scene
            .add_emitter(
                Vector3 {
                    x: pos.x,
                    y: pos.y,
                    z: 0.0,
                },
                EmitterSettings::new(),
            )
            .unwrap();

        let sound_data = sound.data.with_settings(
            StaticSoundSettings::new()
                .start_time(
                    self.clock_handle.time() + (*cur_time - *time_when_started).as_millis() as u64,
                )
                .output_destination(OutputDestination::Emitter(emitter.id())),
        );
        let play = self.manager.play(sound_data).unwrap();
        sound.handle = Some((play, emitter));
    }
}
