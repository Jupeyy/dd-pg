use std::{
    collections::HashMap,
    fmt::Debug,
    sync::{Arc, RwLock},
};

use anyhow::anyhow;
use hashlink::{LinkedHashMap, LinkedHashSet};
use hiarc::Hiarc;
use kira::sound::PlaybackState;

use sound::{
    backend_types::{SoundBackendDriverInterface, SoundBackendMtDriverInterface},
    commands::{
        SceneAirMode, SoundCommand, SoundCommandPlay, SoundCommandSoundListener,
        SoundCommandSoundObject, SoundCommandSoundScene, SoundCommandState, SoundCommandStream,
        SoundCommandStreamObject,
    },
    frame_fetcher_plugin::BackendFrameFetcher,
    sound_mt_types::SoundBackendMemory,
};

use crate::backend::kira::instance::InstanceOffAirProps;

use super::{
    instance::Instance,
    listener::Listener,
    mem_allocator::MemoryAllocator,
    scene::{Scene, ScenePausedState, ScenePausedStateSoundPlay},
    sound::Sound,
    stream::Stream,
};

#[derive(Hiarc)]
pub struct SoundBackendKira {
    main_instance: Instance,
    #[hiarc_skip_unsafe]
    frame_fetchers: Arc<RwLock<HashMap<String, Arc<dyn BackendFrameFetcher>>>>,
    mem_allocator: MemoryAllocator,

    scenes: LinkedHashMap<u128, Scene>,
    inactive_scenes: LinkedHashMap<u128, (Scene, ScenePausedState)>,

    detatched_sound_plays: LinkedHashSet<(u128, u128, u128)>,

    /// strictly monotonic increasing frame id
    cur_sound_frame: u64,
}

impl Debug for SoundBackendKira {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SoundBackendKira").finish()
    }
}

impl SoundBackendKira {
    pub fn new() -> anyhow::Result<Box<Self>> {
        let frame_fetchers: Arc<RwLock<HashMap<String, Arc<dyn BackendFrameFetcher>>>> =
            Default::default();
        let main_instance = Instance::new(frame_fetchers.clone(), None)?;
        Ok(Box::new(Self {
            main_instance,
            frame_fetchers,
            mem_allocator: MemoryAllocator::default(),

            scenes: Default::default(),
            inactive_scenes: Default::default(),

            detatched_sound_plays: Default::default(),

            cur_sound_frame: 0,
        }))
    }
}

impl SoundBackendKira {
    fn stop_detatched_sound_if(
        scenes: &mut LinkedHashMap<u128, Scene>,
        inactive_scenes: &mut LinkedHashMap<u128, (Scene, ScenePausedState)>,
        scene_id: &u128,
        sound_id: &u128,
        play_id: &u128,
        at_state: impl FnOnce(PlaybackState) -> bool,
    ) -> anyhow::Result<bool> {
        let plays = &mut scenes
            .get_mut(scene_id)
            .or(inactive_scenes.get_mut(scene_id).map(|(scene, _)| scene))
            .ok_or_else(|| anyhow!("scene with id {} does not exist", scene_id))?
            .sounds
            .get_mut(sound_id)
            .ok_or_else(|| anyhow!("sound with id {} does not exist.", sound_id))?
            .plays;

        let play = plays
            .get_mut(play_id)
            .ok_or_else(|| anyhow!("sound play with id {play_id} does not exist"))?;

        if at_state(play.handle.state()) {
            plays.remove(play_id);
            anyhow::Ok(true)
        } else {
            anyhow::Ok(false)
        }
    }

    pub fn update(&mut self) -> anyhow::Result<()> {
        self.detatched_sound_plays
            .retain_with_order(
                |(scene_id, sound_id, play_id)| match Self::stop_detatched_sound_if(
                    &mut self.scenes,
                    &mut self.inactive_scenes,
                    scene_id,
                    sound_id,
                    play_id,
                    |state| state == PlaybackState::Stopped,
                ) {
                    Ok(remove) => !remove,
                    Err(_) => false,
                },
            );
        Ok(())
    }

    pub fn get_backend_mt(&self) -> SoundBackendMtKira {
        SoundBackendMtKira {
            mem_allocator: self.mem_allocator.clone(),
        }
    }
}

impl SoundBackendDriverInterface for SoundBackendKira {
    fn run_cmds(&mut self, cmds: Vec<SoundCommand>) -> anyhow::Result<()> {
        // update every frame
        self.update()?;

        for cmd in cmds {
            match cmd {
                SoundCommand::State(cmd) => match cmd {
                    SoundCommandState::SoundScene(cmd) => match cmd {
                        SoundCommandSoundScene::Create { id, props } => {
                            let res = self
                                .scenes
                                .insert(
                                    id,
                                    Scene::new(
                                        match props.air_mode {
                                            SceneAirMode::OnAir => self.main_instance.clone(),
                                            SceneAirMode::OffAir { id, sample_rate } => {
                                                Instance::new(
                                                    self.frame_fetchers.clone(),
                                                    Some(InstanceOffAirProps { id, sample_rate }),
                                                )?
                                            }
                                        },
                                        self.cur_sound_frame,
                                    )?,
                                )
                                .is_none();
                            anyhow::ensure!(res, "scene with that id {id} already existed");
                        }
                        SoundCommandSoundScene::Destroy { id } => {
                            if self.scenes.remove(&id).is_none() {
                                self.inactive_scenes
                                    .remove(&id)
                                    .ok_or_else(|| anyhow!("scene not found."))?;
                            }
                        }
                        SoundCommandSoundScene::StayActive { id } => {
                            // update the sound frame of this id and move scene to end
                            // by moving it to the end later in the [`SoundCommandState::Swap`]
                            // it only needs to check the front scenes
                            if let Some(scene) = self.scenes.get_mut(&id) {
                                scene.last_active_sound_frame = self.cur_sound_frame;

                                self.scenes.to_back(&id);
                            }
                            // if the scene was not active, look in the inactive scenes
                            // active it again and move it to the active scenes
                            else if let Some((mut scene, paused_state)) =
                                self.inactive_scenes.remove(&id)
                            {
                                scene.resume(paused_state)?;
                                scene.last_active_sound_frame = self.cur_sound_frame;
                                self.scenes.insert(id, scene);
                            }
                        }
                        SoundCommandSoundScene::StopDetatchedSounds { id } => {
                            self.detatched_sound_plays.retain_with_order(
                                |(scene_id, sound_id, play_id)| {
                                    if !scene_id.eq(&id) {
                                        return true;
                                    }

                                    if let Err(err) = Self::stop_detatched_sound_if(
                                        &mut self.scenes,
                                        &mut self.inactive_scenes,
                                        scene_id,
                                        sound_id,
                                        play_id,
                                        |_| true,
                                    ) {
                                        log::warn!(target: "kira", "{err}");
                                    }
                                    false
                                },
                            );
                        }
                        SoundCommandSoundScene::ProcessOffAir { id, samples } => {
                            let scene = self
                                .scenes
                                .get_mut(&id)
                                .ok_or_else(|| anyhow!("scene did not exist"))?;

                            scene.instance.process_samples(samples)?;
                        }
                    },
                    SoundCommandState::SoundObject(cmd) => match cmd {
                        SoundCommandSoundObject::Create { id, scene_id, mem } => {
                            let scene = self
                                .scenes
                                .get_mut(&scene_id)
                                .or(self
                                    .inactive_scenes
                                    .get_mut(&scene_id)
                                    .map(|(scene, _)| scene))
                                .ok_or_else(|| anyhow!("scene did not exist"))?;

                            let res = scene
                                .sounds
                                .insert(
                                    id,
                                    Sound::new(&self.mem_allocator, mem)
                                        .map_err(|err| anyhow!("failed to create sound: {err}"))?,
                                )
                                .is_none();
                            anyhow::ensure!(res, "sound with id {id} already existed.");
                        }
                        SoundCommandSoundObject::Destroy { id, scene_id } => {
                            let scene = self
                                .scenes
                                .get_mut(&scene_id)
                                .or(self
                                    .inactive_scenes
                                    .get_mut(&scene_id)
                                    .map(|(scene, _)| scene))
                                .ok_or_else(|| anyhow!("scene did not exist"))?;
                            scene
                                .sounds
                                .remove(&id)
                                .ok_or_else(|| anyhow!("sound not found."))?;
                        }
                    },
                    SoundCommandState::SoundListener(cmd) => match cmd {
                        SoundCommandSoundListener::Create { id, scene_id, pos } => {
                            let scene = self
                                .scenes
                                .get_mut(&scene_id)
                                .or(self
                                    .inactive_scenes
                                    .get_mut(&scene_id)
                                    .map(|(scene, _)| scene))
                                .ok_or_else(|| anyhow!("scene did not exist"))?;

                            let res = scene
                                .listeners
                                .insert(
                                    id,
                                    Listener::new(&mut scene.instance, &mut scene.handle, pos)?,
                                )
                                .is_none();
                            anyhow::ensure!(res, "listener with id {id} already existed.");
                        }
                        SoundCommandSoundListener::Update { id, scene_id, pos } => {
                            let scene = self
                                .scenes
                                .get_mut(&scene_id)
                                .or(self
                                    .inactive_scenes
                                    .get_mut(&scene_id)
                                    .map(|(scene, _)| scene))
                                .ok_or_else(|| anyhow!("scene did not exist"))?;

                            scene
                                .listeners
                                .get_mut(&id)
                                .ok_or_else(|| anyhow!("listener with id {id} did not exist"))?
                                .update(pos);
                        }
                        SoundCommandSoundListener::Destroy { id, scene_id } => {
                            let scene = self
                                .scenes
                                .get_mut(&scene_id)
                                .or(self
                                    .inactive_scenes
                                    .get_mut(&scene_id)
                                    .map(|(scene, _)| scene))
                                .ok_or_else(|| anyhow!("scene did not exist"))?;
                            scene
                                .listeners
                                .remove(&id)
                                .ok_or_else(|| anyhow!("listener not found."))?;
                        }
                    },
                    SoundCommandState::StreamObject(cmd) => match cmd {
                        SoundCommandStreamObject::Create {
                            id,
                            scene_id,
                            stream,
                            props,
                        } => {
                            let (scene, mut paused_state) = self
                                .scenes
                                .get_mut(&scene_id)
                                .map(|sound| (sound, None))
                                .or(self
                                    .inactive_scenes
                                    .get_mut(&scene_id)
                                    .map(|(scene, paused_state)| (scene, Some(paused_state))))
                                .ok_or_else(|| anyhow!("scene did not exist"))?;

                            if let Some(paused_state) = paused_state.as_deref_mut() {
                                paused_state.paused_streams.insert(id);
                            }

                            let res = scene
                                .streams
                                .insert(
                                    id,
                                    Stream::new(
                                        &mut scene.handle,
                                        &scene.instance,
                                        stream.0,
                                        props,
                                        paused_state.is_some(),
                                    )
                                    .map_err(|err| anyhow!("failed to create stream: {err}"))?,
                                )
                                .is_none();
                            anyhow::ensure!(res, "stream with id {id} already existed.");
                        }
                        SoundCommandStreamObject::Destroy { id, scene_id } => {
                            let scene = self
                                .scenes
                                .get_mut(&scene_id)
                                .or(self
                                    .inactive_scenes
                                    .get_mut(&scene_id)
                                    .map(|(scene, _)| scene))
                                .ok_or_else(|| anyhow!("scene did not exist"))?;
                            scene
                                .streams
                                .remove(&id)
                                .ok_or_else(|| anyhow!("stream not found."))?;
                        }
                    },
                    SoundCommandState::Swap => {
                        // check which scenes were inactive and pause those
                        while let Some((&scene_id, scene)) = self.scenes.front() {
                            if scene.is_onair()
                                && scene.last_active_sound_frame < self.cur_sound_frame
                            {
                                if let Some(mut scene) = self.scenes.remove(&scene_id) {
                                    let paused_state = scene.pause()?;
                                    self.inactive_scenes.insert(scene_id, (scene, paused_state));
                                }
                            } else {
                                break;
                            }
                        }

                        self.cur_sound_frame += 1;
                    }
                },
                SoundCommand::Play(cmd) => match cmd {
                    SoundCommandPlay::Play {
                        play_id,
                        sound_id,
                        scene_id,
                        props,
                    } => {
                        let (scene, paused_state) = self
                            .scenes
                            .get_mut(&scene_id)
                            .map(|sound| (sound, None))
                            .or(self
                                .inactive_scenes
                                .get_mut(&scene_id)
                                .map(|(scene, paused_state)| (scene, Some(paused_state))))
                            .ok_or_else(|| anyhow!("scene did not exist"))?;
                        let sound = scene
                            .sounds
                            .get_mut(&sound_id)
                            .ok_or_else(|| anyhow!("sound does not exist."))?;
                        if let Some(paused_state) = paused_state {
                            let non_started_plays = paused_state
                                .non_started_plays
                                .entry(sound_id)
                                .or_insert_with(Default::default);

                            non_started_plays.insert(
                                play_id,
                                ScenePausedStateSoundPlay {
                                    props,
                                    paused: false,
                                },
                            );
                        } else {
                            sound.play(
                                play_id,
                                &mut scene.handle,
                                &scene.instance,
                                props,
                                false,
                            )?;
                        }
                    }
                    SoundCommandPlay::Update {
                        play_id,
                        sound_id,
                        scene_id,
                        props,
                    } => {
                        let (scene, paused_state) = self
                            .scenes
                            .get_mut(&scene_id)
                            .map(|sound| (sound, None))
                            .or(self
                                .inactive_scenes
                                .get_mut(&scene_id)
                                .map(|(scene, paused_state)| (scene, Some(paused_state))))
                            .ok_or_else(|| anyhow!("scene did not exist"))?;
                        let sound = scene
                            .sounds
                            .get_mut(&sound_id)
                            .ok_or_else(|| anyhow!("sound does not exist."))?;
                        if let Some(non_started_sound) = paused_state.and_then(|paused_state| {
                            paused_state
                                .non_started_plays
                                .get_mut(&sound_id)
                                .and_then(|non_started_plays| non_started_plays.get_mut(&play_id))
                        }) {
                            non_started_sound.props.base = props;
                        } else {
                            // TODO: check if updating a sound resumes the sound
                            // because then the `paused_plays` sound must get a "pending update"
                            // property.
                            sound.update(play_id, props)?;
                        }
                    }
                    SoundCommandPlay::Pause {
                        play_id,
                        sound_id,
                        scene_id,
                    } => {
                        let (scene, paused_state) = self
                            .scenes
                            .get_mut(&scene_id)
                            .map(|sound| (sound, None))
                            .or(self
                                .inactive_scenes
                                .get_mut(&scene_id)
                                .map(|(scene, paused_state)| (scene, Some(paused_state))))
                            .ok_or_else(|| anyhow!("scene did not exist"))?;
                        let sound = scene
                            .sounds
                            .get_mut(&sound_id)
                            .ok_or_else(|| anyhow!("sound does not exist."))?;
                        if let Some(paused_state) = paused_state {
                            if let Some(non_started_plays) = paused_state
                                .non_started_plays
                                .get_mut(&sound_id)
                                .and_then(|plays| plays.get_mut(&play_id))
                            {
                                non_started_plays.paused = true;
                            } else {
                                anyhow::ensure!(
                                    paused_state
                                        .paused_plays
                                        .get(&sound_id)
                                        .is_some_and(|plays| plays.contains(&play_id)),
                                    "no sound play with id {sound_id} - {play_id} found."
                                );
                            }
                        } else {
                            sound.pause(play_id)?;
                        }
                    }
                    SoundCommandPlay::Resume {
                        play_id,
                        sound_id,
                        scene_id,
                    } => {
                        let (scene, paused_state) = self
                            .scenes
                            .get_mut(&scene_id)
                            .map(|sound| (sound, None))
                            .or(self
                                .inactive_scenes
                                .get_mut(&scene_id)
                                .map(|(scene, paused_state)| (scene, Some(paused_state))))
                            .ok_or_else(|| anyhow!("scene did not exist"))?;
                        let sound = scene
                            .sounds
                            .get_mut(&sound_id)
                            .ok_or_else(|| anyhow!("sound does not exist."))?;
                        if let Some(paused_state) = paused_state {
                            if let Some(non_started_plays) = paused_state
                                .non_started_plays
                                .get_mut(&sound_id)
                                .and_then(|plays| plays.get_mut(&play_id))
                            {
                                non_started_plays.paused = false;
                            } else {
                                anyhow::ensure!(
                                    paused_state
                                        .paused_plays
                                        .get(&sound_id)
                                        .is_some_and(|plays| plays.contains(&play_id)),
                                    "no sound play with id {sound_id} - {play_id} found."
                                );
                            }
                        } else {
                            sound.resume(play_id)?;
                        }
                    }
                    SoundCommandPlay::Detatch {
                        play_id,
                        sound_id,
                        scene_id,
                    } => {
                        let res = self
                            .detatched_sound_plays
                            .insert((scene_id, sound_id, play_id));
                        anyhow::ensure!(res, "sound play with id {play_id} already existed.");
                    }
                    SoundCommandPlay::Stop {
                        play_id,
                        sound_id,
                        scene_id,
                    } => {
                        let (scene, paused_state) = self
                            .scenes
                            .get_mut(&scene_id)
                            .map(|sound| (sound, None))
                            .or(self
                                .inactive_scenes
                                .get_mut(&scene_id)
                                .map(|(scene, paused_state)| (scene, Some(paused_state))))
                            .ok_or_else(|| anyhow!("scene did not exist"))?;
                        let sound = scene
                            .sounds
                            .get_mut(&sound_id)
                            .ok_or_else(|| anyhow!("sound does not exist."))?;
                        if let Some(paused_state) = paused_state {
                            if let Some(paused_plays) = paused_state.paused_plays.get_mut(&sound_id)
                            {
                                paused_plays.remove(&play_id);
                                if paused_plays.is_empty() {
                                    paused_state.paused_plays.remove(&sound_id);
                                }
                            };
                            if let Some(non_started_plays) =
                                paused_state.non_started_plays.get_mut(&sound_id)
                            {
                                non_started_plays.remove(&play_id);
                                if non_started_plays.is_empty() {
                                    paused_state.non_started_plays.remove(&sound_id);
                                }
                            }
                        } else {
                            sound.plays.remove(&play_id).ok_or_else(|| {
                                anyhow!("sound play with id {play_id} did not exist")
                            })?;
                        }
                    }
                },
                SoundCommand::Stream(cmd) => match cmd {
                    SoundCommandStream::Update {
                        stream_id,
                        scene_id,
                        props,
                    } => {
                        let (scene, paused_state) = self
                            .scenes
                            .get_mut(&scene_id)
                            .map(|sound| (sound, None))
                            .or(self
                                .inactive_scenes
                                .get_mut(&scene_id)
                                .map(|(scene, paused_state)| (scene, Some(paused_state))))
                            .ok_or_else(|| anyhow!("scene did not exist"))?;

                        let stream = scene
                            .streams
                            .get_mut(&stream_id)
                            .ok_or_else(|| anyhow!("stream does not exist."))?;
                        stream.update(props, paused_state.is_some());
                    }
                    SoundCommandStream::Pause {
                        stream_id,
                        scene_id,
                    } => {
                        let scene = self
                            .scenes
                            .get_mut(&scene_id)
                            .or(self
                                .inactive_scenes
                                .get_mut(&scene_id)
                                .map(|(scene, _)| scene))
                            .ok_or_else(|| anyhow!("scene did not exist"))?;

                        let stream = scene
                            .streams
                            .get_mut(&stream_id)
                            .ok_or_else(|| anyhow!("stream does not exist."))?;
                        stream.pause(true);
                    }
                    SoundCommandStream::Resume {
                        stream_id,
                        scene_id,
                    } => {
                        let (scene, paused_state) = self
                            .scenes
                            .get_mut(&scene_id)
                            .map(|sound| (sound, None))
                            .or(self
                                .inactive_scenes
                                .get_mut(&scene_id)
                                .map(|(scene, paused_state)| (scene, Some(paused_state))))
                            .ok_or_else(|| anyhow!("scene did not exist"))?;

                        let stream = scene
                            .streams
                            .get_mut(&stream_id)
                            .ok_or_else(|| anyhow!("stream does not exist."))?;
                        stream.resume(paused_state.is_some(), true);
                    }
                },
            }
        }

        Ok(())
    }

    fn attach_frame_fetcher(&mut self, name: String, fetcher: Arc<dyn BackendFrameFetcher>) {
        self.frame_fetchers.write().unwrap().insert(name, fetcher);
    }

    fn detach_frame_fetcher(&mut self, name: String) {
        self.frame_fetchers.write().unwrap().remove(&name);
    }
}

#[derive(Debug, Hiarc)]
pub struct SoundBackendMtKira {
    mem_allocator: MemoryAllocator,
}

impl SoundBackendMtDriverInterface for SoundBackendMtKira {
    fn mem_alloc(&self, size: usize) -> SoundBackendMemory {
        self.mem_allocator.mem_alloc(size)
    }

    fn try_flush_mem(&self, mem: &mut SoundBackendMemory) -> anyhow::Result<()> {
        self.mem_allocator.try_flush_mem(mem)
    }
}
