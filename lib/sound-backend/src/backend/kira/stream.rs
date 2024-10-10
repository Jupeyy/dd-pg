mod stream_decoder;

use std::{fmt::Debug, sync::Arc};

use hiarc::Hiarc;
use kira::{
    sound::streaming::{StreamingSoundData, StreamingSoundHandle, StreamingSoundSettings},
    spatial::{
        emitter::{EmitterDistances, EmitterHandle, EmitterSettings},
        scene::SpatialSceneHandle,
    },
    tween::Easing,
    OutputDestination,
};
use mint::Vector3;

use sound::{
    stream::{self},
    types::{StreamPlayBaseProps, StreamPlayProps},
};
use stream_decoder::StreamDecoder;

use super::instance::Instance;

#[derive(Hiarc)]
pub(super) struct Stream {
    pub(super) stream_handle: StreamingSoundHandle<anyhow::Error>,
    pub(super) volume: f64,
    /// paused by a call to [`Self::pause`]
    pub(super) self_paused: bool,

    pub(super) _emitter: EmitterHandle,
}

impl Stream {
    pub fn new(
        scene: &mut SpatialSceneHandle,
        instance: &Instance,
        decoder: Arc<dyn stream::StreamDecoder>,
        props: StreamPlayProps,
        paused: bool,
    ) -> anyhow::Result<Self> {
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
                .attenuation_function(props.pow_attenuation_value.map(Easing::InPowf))
                .persist_until_sounds_finish(false),
        )?;

        let sound_settings = StreamingSoundSettings::new()
            .start_time(instance.clock_time())
            .volume(props.base.volume)
            .panning(props.base.panning)
            .output_destination(OutputDestination::Emitter(emitter.id()));

        let sound_data = StreamingSoundData::from_decoder(StreamDecoder { decoder });

        let sound_data = sound_data.with_settings(sound_settings);
        let mut stream_handle = instance.play_stream(sound_data)?;

        if paused {
            stream_handle.set_volume(0.0, Default::default());
        }

        Ok(Self {
            _emitter: emitter,
            stream_handle,
            volume: props.base.volume,
            self_paused: false,
        })
    }

    pub fn pause(&mut self, self_pause: bool) {
        if self_pause {
            self.self_paused = true;
        }
        self.stream_handle.set_volume(0.0, Default::default());
    }

    pub fn resume(&mut self, scene_paused: bool, self_resume: bool) {
        if self_resume {
            self.self_paused = false;
        }
        if !scene_paused {
            self.stream_handle
                .set_volume(self.volume, Default::default());
        }
    }

    pub fn update(&mut self, props: StreamPlayBaseProps, scene_paused: bool) {
        self._emitter.set_position(
            Vector3 {
                x: props.pos.x,
                y: props.pos.y,
                z: 0.0,
            },
            Default::default(),
        );

        if !scene_paused && !self.self_paused {
            self.stream_handle
                .set_volume(props.volume, Default::default());
        }
        // keep the volume for pausing
        self.volume = props.volume;
        self.stream_handle
            .set_panning(props.panning, Default::default());
    }
}

impl Debug for Stream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Sound").finish()
    }
}
