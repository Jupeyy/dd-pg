use std::fmt::Debug;

use anyhow::anyhow;
use hashlink::LinkedHashMap;
use hiarc::Hiarc;
use kira::{
    clock::ClockHandle,
    sound::static_sound::{StaticSoundData, StaticSoundHandle, StaticSoundSettings},
    spatial::{
        emitter::{EmitterDistances, EmitterHandle, EmitterSettings},
        scene::SpatialSceneHandle,
    },
    tween::Easing,
    OutputDestination,
};
use mint::Vector3;

use sound::{
    sound_mt_types::SoundBackendMemory,
    types::{SoundPlayBaseProps, SoundPlayProps},
};

use super::{instance::Instance, mem_allocator::MemoryAllocator};

#[derive(Hiarc)]
/// actually playing sound in the scene
pub(super) struct SoundPlay {
    pub(super) handle: StaticSoundHandle,
    pub(super) emitter: EmitterHandle,
    //_start_time: Duration,
}

impl Debug for SoundPlay {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SoundPlay").finish()
    }
}

#[derive(Hiarc)]
pub(super) struct Sound {
    pub(super) sound_data: StaticSoundData,

    pub(super) plays: LinkedHashMap<u128, SoundPlay>,
}

impl Sound {
    pub fn new(mem_allocator: &MemoryAllocator, mem: SoundBackendMemory) -> anyhow::Result<Self> {
        let sound_data = mem_allocator.sound_data_from_mem(mem)?;

        Ok(Self {
            sound_data,
            plays: Default::default(),
        })
    }

    pub fn play(
        &mut self,
        play_id: u128,
        scene: &mut SpatialSceneHandle,
        clock_handle: &ClockHandle,
        instance: &Instance,
        props: SoundPlayProps,
        paused: bool,
    ) -> anyhow::Result<()> {
        let emitter = scene.add_emitter(
            Vector3 {
                x: props.base.pos.x,
                y: props.base.pos.y,
                z: 0.0,
            },
            EmitterSettings::new()
                .distances(EmitterDistances {
                    min_distance: props.min_distance,
                    max_distance: props.max_distance,
                })
                .enable_spatialization(props.spartial)
                .attenuation_function(props.pow_attenuation_value.map(Easing::InPowf)),
        )?;

        let mut sound_settings = StaticSoundSettings::new()
            .start_time(clock_handle.time() + props.start_time_delay.as_millis() as u64)
            .volume(props.base.volume)
            .panning(props.base.panning)
            .output_destination(OutputDestination::Emitter(emitter.id()));

        if props.base.looped {
            sound_settings = sound_settings.loop_region(..);
        }

        let sound_data = self.sound_data.with_settings(sound_settings);
        let mut play = instance.play(sound_data)?;
        if paused {
            play.pause(Default::default());
        }
        let res = self
            .plays
            .insert(
                play_id,
                SoundPlay {
                    handle: play,
                    emitter,
                },
            )
            .is_none();
        anyhow::ensure!(
            res,
            "sound play handle with that id {play_id} already existed."
        );

        Ok(())
    }

    pub fn pause(&mut self, play_id: u128) -> anyhow::Result<()> {
        let play = self
            .plays
            .get_mut(&play_id)
            .ok_or_else(|| anyhow!("sound with id {} did not exist", play_id))?;
        play.handle.pause(Default::default());
        Ok(())
    }

    pub fn resume(&mut self, play_id: u128) -> anyhow::Result<()> {
        let play = self
            .plays
            .get_mut(&play_id)
            .ok_or_else(|| anyhow!("sound with id {} did not exist", play_id))?;
        play.handle.resume(Default::default());
        Ok(())
    }

    pub fn update(&mut self, play_id: u128, props: SoundPlayBaseProps) -> anyhow::Result<()> {
        let play = self
            .plays
            .get_mut(&play_id)
            .ok_or_else(|| anyhow!("sound with id {} did not exist", play_id))?;

        play.emitter.set_position(
            Vector3 {
                x: props.pos.x,
                y: props.pos.y,
                z: 0.0,
            },
            Default::default(),
        );

        play.handle.set_volume(props.volume, Default::default());
        play.handle.set_panning(props.panning, Default::default());
        if props.looped {
            play.handle.set_loop_region(..);
        } else {
            play.handle.set_loop_region(None);
        }

        Ok(())
    }
}

impl Debug for Sound {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Sound").finish()
    }
}
