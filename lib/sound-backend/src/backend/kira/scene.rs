use std::fmt::Debug;

use hashlink::LinkedHashMap;
use hiarc::Hiarc;
use kira::spatial::scene::SpatialSceneHandle;

use super::{instance::Instance, listener::Listener, sound::Sound, stream::Stream};

#[derive(Hiarc)]
pub struct SceneActive {
    pub(super) handle: SpatialSceneHandle,
}

impl Debug for SceneActive {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SceneActive").finish()
    }
}

#[derive(Debug, Hiarc)]
pub enum SceneState {
    Active(SceneActive),
    ForcePaused,
    Paused,
}

impl SceneState {
    pub fn scene_handle(&mut self) -> Option<&mut SpatialSceneHandle> {
        match self {
            SceneState::Active(scene_active) => Some(&mut scene_active.handle),
            SceneState::ForcePaused => None,
            SceneState::Paused => None,
        }
    }
}

#[derive(Debug, Hiarc)]
pub(super) struct Scene {
    pub(super) sounds: LinkedHashMap<u128, Sound>,
    pub(super) streams: LinkedHashMap<u128, Stream>,
    pub(super) listeners: LinkedHashMap<u128, Listener>,

    pub(super) instance: Instance,
    pub(super) state: SceneState,

    pub(super) last_active_sound_frame: u64,
}

impl Scene {
    pub fn new(instance: Instance, cur_sound_frame: u64) -> anyhow::Result<Self> {
        let state = match instance.add_spatial_scene() {
            Ok(scene) => SceneState::Active(SceneActive { handle: scene }),
            Err(_) => SceneState::ForcePaused,
        };

        Ok(Self {
            sounds: Default::default(),
            streams: Default::default(),
            listeners: Default::default(),
            last_active_sound_frame: cur_sound_frame,

            instance,
            state,
        })
    }

    /// Returns whether the scene was paused.
    pub fn pause(&mut self) -> anyhow::Result<bool> {
        match &mut self.state {
            SceneState::Active(_) | SceneState::ForcePaused => {
                self.state = SceneState::Paused;

                for sound in self.sounds.values_mut() {
                    let play_ids: Vec<_> = sound.plays.keys().cloned().collect();
                    for play_id in play_ids {
                        sound.force_pause(play_id, &self.instance)?;
                    }
                }
                for stream in self.streams.values_mut() {
                    stream.force_pause();
                }
                for listener in self.listeners.values_mut() {
                    listener.handle = None;
                }
                Ok(true)
            }
            SceneState::Paused => {
                // nothing to do
                Ok(false)
            }
        }
    }

    pub fn resume(&mut self) -> anyhow::Result<()> {
        match &self.state {
            SceneState::Active(_) => {
                // nothing to do
            }
            SceneState::ForcePaused | SceneState::Paused => {
                // first try to resume the scene itself
                match self.instance.add_spatial_scene() {
                    Ok(mut scene) => {
                        for sound in self.sounds.values_mut() {
                            let play_ids: Vec<_> = sound.plays.keys().cloned().collect();
                            for play_id in play_ids {
                                sound.resume(play_id, Some(&mut scene), &self.instance)?;
                            }
                        }
                        for stream in self.streams.values_mut() {
                            stream.resume(Some(&mut scene), &self.instance);
                        }
                        for listener in self.listeners.values_mut() {
                            listener.reattach_to_scene(&mut scene, &self.instance);
                        }

                        self.state = SceneState::Active(SceneActive { handle: scene });
                    }
                    Err(_) => {
                        self.state = SceneState::ForcePaused;
                    }
                }
            }
        }
        Ok(())
    }

    pub fn is_onair(&self) -> bool {
        self.instance.is_onair()
    }
}
