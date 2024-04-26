use std::fmt::Debug;

use hashlink::{LinkedHashMap, LinkedHashSet};
use hiarc::Hiarc;
use kira::{
    clock::ClockHandle, sound::PlaybackState, spatial::scene::SpatialSceneHandle, tween::Tween,
};
use sound::types::SoundPlayProps;

use super::{instance::Instance, listener::Listener, sound::Sound};

/// the state when the scene was paused
#[derive(Debug, Hiarc, Default)]
pub struct ScenePausedState {
    pub paused_plays: LinkedHashMap<u128, LinkedHashSet<u128>>,
    pub non_started_plays: LinkedHashMap<u128, LinkedHashMap<u128, SoundPlayProps>>,
}

#[derive(Hiarc)]
pub(super) struct Scene {
    pub(super) sounds: LinkedHashMap<u128, Sound>,
    pub(super) listeners: LinkedHashMap<u128, Listener>,
    pub(super) handle: SpatialSceneHandle,

    pub(super) last_active_sound_frame: u64,
}

impl Scene {
    pub fn new(instance: Instance, cur_sound_frame: u64) -> anyhow::Result<Self> {
        let scene = instance.add_spatial_scene()?;
        Ok(Self {
            sounds: Default::default(),
            listeners: Default::default(),
            handle: scene,
            last_active_sound_frame: cur_sound_frame,
        })
    }

    pub fn pause(&mut self) -> anyhow::Result<ScenePausedState> {
        let mut pause_state = ScenePausedState::default();
        for (sound_id, sound) in self.sounds.iter_mut() {
            for (play_id, play) in sound.plays.iter_mut() {
                if play.handle.state() == PlaybackState::Playing {
                    play.handle.pause(Tween::default())?;
                    if let Some(paused_plays) = pause_state.paused_plays.get_mut(sound_id) {
                        paused_plays.insert(*play_id);
                    } else {
                        let mut plays = LinkedHashSet::default();
                        plays.insert(*play_id);
                        pause_state.paused_plays.insert(*sound_id, plays);
                    }
                }
            }
        }
        Ok(pause_state)
    }

    pub fn resume(
        &mut self,
        pause_state: ScenePausedState,
        clock_handle: &ClockHandle,
        instance: &Instance,
    ) -> anyhow::Result<()> {
        for (sound_id, play_ids) in pause_state.paused_plays {
            if let Some(sound) = self.sounds.get_mut(&sound_id) {
                for play_id in play_ids {
                    if let Some(play) = sound.plays.get_mut(&play_id) {
                        play.handle.resume(Tween::default())?;
                    }
                }
            }
        }
        for (sound_id, play_ids) in pause_state.non_started_plays {
            if let Some(sound) = self.sounds.get_mut(&sound_id) {
                for (play_id, props) in play_ids {
                    sound.play(play_id, &mut self.handle, clock_handle, instance, props)?;
                }
            }
        }
        Ok(())
    }
}

impl Debug for Scene {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Scene")
            .field("sounds", &self.sounds)
            .finish()
    }
}
