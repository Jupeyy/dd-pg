use std::fmt::Debug;

use hashlink::LinkedHashMap;
use hiarc::Hiarc;
use kira::{
    clock::ClockHandle,
    sound::static_sound::{StaticSoundData, StaticSoundHandle, StaticSoundSettings},
    spatial::{
        emitter::{EmitterHandle, EmitterSettings},
        scene::SpatialSceneHandle,
    },
    OutputDestination,
};
use mint::Vector3;

use sound::{sound_mt_types::SoundBackendMemory, types::SoundPlayProps};

use super::{instance::Instance, mem_allocator::MemoryAllocator};

#[derive(Hiarc)]
/// actually playing sound in the scene
pub(super) struct SoundPlay {
    pub(super) handle: StaticSoundHandle,
    pub(super) _emitter: EmitterHandle,
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
    ) -> anyhow::Result<()> {
        let emitter = scene.add_emitter(
            Vector3 {
                x: props.pos.x,
                y: props.pos.y,
                z: 0.0,
            },
            EmitterSettings::new(),
        )?;

        let mut sound_settings = StaticSoundSettings::new()
            .start_time(clock_handle.time() + props.time_offset.as_millis() as u64)
            .output_destination(OutputDestination::Emitter(emitter.id()));

        if props.looped {
            sound_settings = sound_settings.loop_region(..);
        }

        let sound_data = self.sound_data.with_settings(sound_settings);
        let play = instance.play(sound_data)?;
        let res = self
            .plays
            .insert(
                play_id,
                SoundPlay {
                    handle: play,
                    _emitter: emitter,
                },
            )
            .is_none();
        anyhow::ensure!(
            res,
            "sound play handle with that id {play_id} already existed."
        );

        Ok(())
    }
}

impl Debug for Sound {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Sound").finish()
    }
}
