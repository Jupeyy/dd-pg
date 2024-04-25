use std::fmt::Debug;

use hiarc::{hiarc_safer_rc_refcell, Hiarc};
use kira::{
    clock::{ClockHandle, ClockSpeed},
    manager::{backend::DefaultBackend, AudioManager, AudioManagerSettings, Capacities},
    sound::static_sound::{StaticSoundData, StaticSoundHandle},
    spatial::scene::{SpatialSceneHandle, SpatialSceneSettings},
};

#[hiarc_safer_rc_refcell]
#[derive(Hiarc)]
pub struct Instance {
    manager: AudioManager<DefaultBackend>,
}

#[hiarc_safer_rc_refcell]
impl Instance {
    pub fn new() -> anyhow::Result<Self> {
        let settings = AudioManagerSettings::<DefaultBackend> {
            capacities: Capacities {
                command_capacity: 8192,
                sound_capacity: 8192,
                sub_track_capacity: 8192,
                clock_capacity: 64,
                spatial_scene_capacity: 128,
                modulator_capacity: 64,
            },
            main_track_builder: Default::default(),
            backend_settings: Default::default(),
        };
        let manager = AudioManager::<DefaultBackend>::new(settings)?;
        Ok(Self { manager })
    }

    pub fn add_clock(&mut self) -> anyhow::Result<ClockHandle> {
        let clock = self
            .manager
            .add_clock(ClockSpeed::TicksPerMinute(1000.0 * 60.0))?;
        clock.start()?;
        Ok(clock)
    }

    pub fn add_spatial_scene(&mut self) -> anyhow::Result<SpatialSceneHandle> {
        Ok(self
            .manager
            .add_spatial_scene(SpatialSceneSettings::new())?)
    }

    pub fn play(&mut self, sound_data: StaticSoundData) -> anyhow::Result<StaticSoundHandle> {
        Ok(self.manager.play(sound_data)?)
    }
}

impl Debug for Instance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Instance").finish()
    }
}
