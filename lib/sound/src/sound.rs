use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use base::system::{System, SystemTimeInterface};
use base_fs::{
    filesys::FileSystem,
    io_batcher::{TokIOBatcher, TokIOBatcherTask},
};
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
use math::math::vector::vec2;

pub struct SoundLoading {
    task: Option<TokIOBatcherTask<Vec<u8>>>,
}

pub struct Sound {
    data: StaticSoundData,
    handle: Option<StaticSoundHandle>,
    start_time: Duration,
}

pub struct SoundQueued {
    start_time: Duration,
}

pub struct SoundListener {
    pos: vec2,
}

pub struct SoundManager {
    files: LinkedHashMap<String, Sound>,
    files_loading: LinkedHashMap<String, SoundLoading>,
    manager: AudioManager<DefaultBackend>,
    clock_handle: ClockHandle,
    scene: SpatialSceneHandle,

    queued_for_playing: LinkedHashMap<String, SoundQueued>,
    queued_for_playing_helper: LinkedHashMap<String, SoundQueued>,
}

impl SoundManager {
    pub fn new() -> Self {
        let mut manager =
            AudioManager::<DefaultBackend>::new(AudioManagerSettings::default()).unwrap(); // TODO
        let clock_handle = manager
            .add_clock(ClockSpeed::TicksPerMinute(1000.0 * 60.0))
            .unwrap();
        let scene = manager
            .add_spatial_scene(SpatialSceneSettings::new())
            .unwrap();
        Self {
            files: Default::default(),
            files_loading: Default::default(),
            manager,
            clock_handle,
            scene,

            queued_for_playing: Default::default(),
            queued_for_playing_helper: Default::default(),
        }
    }

    pub fn destroy(mut self, io_batcher: &Arc<Mutex<TokIOBatcher>>) {
        // force finish loading items
        self.files_loading
            .drain()
            .for_each(|(_, mut loading_file)| {
                if let Some(mut sound) = loading_file.task.take() {
                    io_batcher
                        .lock()
                        .unwrap()
                        .wait_finished_and_drop(&mut sound);
                }
            });
    }

    pub fn update(
        &mut self,
        io_batcher: &Arc<Mutex<TokIOBatcher>>,
        sys: &System,
        listeners: &[SoundListener],
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
                    io_batcher,
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
        let mut listener = self
            .scene
            .add_listener(pos, orientation, ListenerSettings::new())
            .unwrap();
    }

    pub fn load_sound_file(
        &mut self,
        name: &str,
        fs: &Arc<FileSystem>,
        io_batcher: &Arc<Mutex<TokIOBatcher>>,
    ) {
        let sound_name = name.to_string();
        let sound_name_thread = name.to_string();
        let fs = fs.clone();
        let task = io_batcher.lock().unwrap().spawn(async move {
            let file_path = fs
                .open_file(&("sound/".to_string() + &sound_name_thread + ".ogg"))
                .await;
            Ok(file_path?)
        });
        self.files_loading
            .insert(sound_name, SoundLoading { task: Some(task) });
    }

    fn check_loading(&mut self, sound: &str, io_batcher: &Arc<Mutex<TokIOBatcher>>) -> bool {
        // check for loading sounds first
        let mut was_loading = false;
        if let Some(loading_file) = self.files_loading.get_mut(sound) {
            if let Some(mut task) = loading_file.task.take() {
                if task.is_finished() {
                    io_batcher.lock().unwrap().wait_finished_and_drop(&mut task);
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
    fn play_at(
        &mut self,
        sound: &str,
        io_batcher: &Arc<Mutex<TokIOBatcher>>,
        time_when_started: &Duration,
        cur_time: &Duration,
    ) {
        let was_loading = self.check_loading(sound, io_batcher);

        if let Some(sound) = self.files.get_mut(sound) {
            let sound_data = sound.data.with_settings(
                StaticSoundSettings::new()
                    .start_time(
                        self.clock_handle.time()
                            + (*cur_time - *time_when_started).as_millis() as u64,
                    )
                    .output_destination(OutputDestination::MAIN_TRACK),
            );
            let play = self.manager.play(sound_data).unwrap();
            sound.handle = Some(play);
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

    pub fn play(&mut self, sound: &str, io_batcher: &Arc<Mutex<TokIOBatcher>>, sys: &System) {
        let cur_time = sys.time_get_nanoseconds();
        self.play_at(sound, io_batcher, &cur_time, &cur_time.clone());
    }

    pub fn load_sound(&mut self, sound_name: &str, file_data: Vec<u8>) {
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
                start_time: Duration::ZERO,
            },
        );
    }
}
