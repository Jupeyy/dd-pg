use std::{sync::Arc, time::Duration};

use base::system::{System, SystemTimeInterface};
use base_fs::{
    filesys::FileSystem,
    io_batcher::{TokIOBatcher, TokIOBatcherTask},
};
use base_fs_traits::traits::FileSystemInterface;
use hashlink::LinkedHashMap;

use math::math::vector::vec2;

use crate::{
    backend::{kiri::kiri::SoundBackendKira, null::null::SoundBackendNull},
    backend_types::SoundBackend,
};

pub struct SoundLoading {
    task: Option<TokIOBatcherTask<Vec<u8>>>,
}

pub struct SoundQueued {
    start_time: Duration,
}

pub struct SoundListener {
    _pos: vec2,
}

pub struct SoundManager {
    backend: SoundBackend,

    files_loading: LinkedHashMap<String, SoundLoading>,

    queued_for_playing: LinkedHashMap<String, SoundQueued>,
    queued_for_playing_helper: LinkedHashMap<String, SoundQueued>,
}

impl SoundManager {
    pub fn new() -> Self {
        let backend = match SoundBackendKira::new() {
            Ok(backend) => SoundBackend::Kira(backend),
            Err(_) => SoundBackend::None(SoundBackendNull {}),
        };

        Self {
            files_loading: Default::default(),

            backend,

            queued_for_playing: Default::default(),
            queued_for_playing_helper: Default::default(),
        }
    }

    pub fn update(
        &mut self,
        _io_batcher: &TokIOBatcher,
        sys: &System,
        _listeners: &[SoundListener],
    ) {
        // TODO: retain with order? does order matter for sounds realistically?
        let mut queued_for_playing_helper = Default::default();
        // use helper memory
        std::mem::swap(
            &mut queued_for_playing_helper,
            &mut self.queued_for_playing_helper,
        );
        std::mem::swap(&mut self.queued_for_playing, &mut queued_for_playing_helper);
        queued_for_playing_helper
            .drain()
            .for_each(|(sound_name, queued_sound)| {
                self.play_at(
                    &sound_name,
                    &queued_sound.start_time,
                    &sys.time_get_nanoseconds(),
                );
            });
        // give helper his memory back
        std::mem::swap(
            &mut queued_for_playing_helper,
            &mut self.queued_for_playing_helper,
        );

        let pos: mint::Vector3<f32> = mint::Vector3 {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        };
        let orientation: mint::Quaternion<f32> = mint::Quaternion {
            s: 1.0,
            v: mint::Vector3 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
        };
        let _listener = self.backend.as_mut().add_listener(pos, orientation);
    }

    pub fn load_sound_file(&mut self, name: &str, fs: &Arc<FileSystem>, io_batcher: &TokIOBatcher) {
        let sound_name = name.to_string();
        let sound_name_thread = name.to_string();
        let fs = fs.clone();
        let task = io_batcher.spawn(async move {
            let file_path = fs
                .open_file(&("sound/".to_string() + &sound_name_thread + ".ogg"))
                .await;
            Ok(file_path?)
        });
        self.files_loading
            .insert(sound_name, SoundLoading { task: Some(task) });
    }

    fn check_loading(&mut self, sound: &str) -> bool {
        // check for loading sounds first
        let mut was_loading = false;
        if let Some(loading_file) = self.files_loading.get_mut(sound) {
            if let Some(task) = loading_file.task.take() {
                if task.is_finished() {
                    let sound_file = task.get_storage();
                    if let Ok(sound_data) = sound_file {
                        self.load_sound(sound, sound_data);
                    }
                } else {
                    let _ = loading_file.task.insert(task);
                }
                was_loading = true;
            }
        }
        was_loading
    }

    /// `time_when_started` is the time when the sound actually started, `cur_time` uses the difference to this
    /// to evaluate the passed time
    fn play_at(&mut self, sound: &str, time_when_started: &Duration, cur_time: &Duration) {
        let was_loading = self.check_loading(sound);

        if self.backend.as_mut().sound_exists(sound) {
            self.backend
                .as_mut()
                .play_at(sound, time_when_started, cur_time);
        } else if was_loading {
            // still insert into playing sounds, but also make sure that the file is played as soon as possible
            self.queued_for_playing.insert(
                sound.to_string(),
                SoundQueued {
                    start_time: *time_when_started,
                },
            );
        }
    }

    pub fn play(&mut self, sound: &str, sys: &System) {
        let cur_time = sys.time_get_nanoseconds();
        self.play_at(sound, &cur_time, &cur_time.clone());
    }

    pub fn load_sound(&mut self, sound_name: &str, file_data: Vec<u8>) {
        self.backend.as_mut().load_sound(sound_name, file_data)
    }
}
