use std::fmt::Debug;

use anyhow::anyhow;
use hashlink::{LinkedHashMap, LinkedHashSet};
use hiarc::Hiarc;
use kira::{clock::ClockHandle, sound::PlaybackState};

use sound::{
    backend_types::{SoundBackendDriverInterface, SoundBackendMtDriverInterface},
    commands::{
        SoundCommand, SoundCommandPlay, SoundCommandSoundListener, SoundCommandSoundObject,
        SoundCommandSoundScene, SoundCommandState,
    },
    sound_mt_types::SoundBackendMemory,
};

use super::{
    instance::Instance,
    listener::Listener,
    mem_allocator::MemoryAllocator,
    scene::{Scene, ScenePausedState, ScenePausedStateSoundPlay},
    sound::Sound,
};

#[derive(Hiarc)]
pub struct SoundBackendKira {
    instance: Instance,
    mem_allocator: MemoryAllocator,

    clock_handle: ClockHandle,
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
        let instance = Instance::new()?;
        let clock_handle = instance.add_clock()?;
        Ok(Box::new(Self {
            instance,
            mem_allocator: MemoryAllocator::new(),

            clock_handle,
            scenes: Default::default(),
            inactive_scenes: Default::default(),

            detatched_sound_plays: Default::default(),

            cur_sound_frame: 0,
        }))
    }
}

impl SoundBackendKira {
    pub fn update(&mut self) -> anyhow::Result<()> {
        self.detatched_sound_plays
            .retain_with_order(|(scene_id, sound_id, play_id)| {
                let mut play = || {
                    let plays = &mut self
                        .scenes
                        .get_mut(scene_id)
                        .or(self
                            .inactive_scenes
                            .get_mut(scene_id)
                            .map(|(scene, _)| scene))
                        .ok_or_else(|| anyhow!("scene with id {} does not exist", scene_id))?
                        .sounds
                        .get_mut(sound_id)
                        .ok_or_else(|| anyhow!("sound with id {} does not exist.", sound_id))?
                        .plays;

                    let play = plays
                        .get_mut(play_id)
                        .ok_or_else(|| anyhow!("sound play with id {play_id} does not exist"))?;

                    if play.handle.state() == PlaybackState::Stopped {
                        plays.remove(play_id);
                        anyhow::Ok(true)
                    } else {
                        anyhow::Ok(false)
                    }
                };
                match play() {
                    Ok(remove) => !remove,
                    Err(_) => false,
                }
            });
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
                        SoundCommandSoundScene::Create { id } => {
                            let res = self
                                .scenes
                                .insert(
                                    id,
                                    Scene::new(self.instance.clone(), self.cur_sound_frame)?,
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
                                scene.resume(paused_state, &self.clock_handle, &self.instance)?;
                                scene.last_active_sound_frame = self.cur_sound_frame;
                                self.scenes.insert(id, scene);
                            }
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
                                .insert(id, Listener::new(&mut scene.handle, pos)?)
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
                    SoundCommandState::Swap => {
                        // check which scenes were inactive and pause those
                        while let Some((&scene_id, scene)) = self.scenes.front() {
                            if scene.last_active_sound_frame < self.cur_sound_frame {
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
                                &self.clock_handle,
                                &self.instance,
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
            }
        }

        Ok(())
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
