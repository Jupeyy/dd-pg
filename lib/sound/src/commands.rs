use std::sync::Arc;

use hiarc::Hiarc;
use math::math::vector::vec2;
use serde::{Deserialize, Serialize};

use crate::{
    sound_mt_types::SoundBackendMemory,
    stream::StreamDecoder,
    types::{SoundPlayBaseProps, SoundPlayProps, StreamPlayBaseProps, StreamPlayProps},
};

#[derive(Debug, Hiarc, Copy, Clone, Default, Serialize, Deserialize)]
pub enum SceneAirMode {
    #[default]
    OnAir,
    OffAir {
        id: u64,
        sample_rate: u32,
    },
}

#[derive(Debug, Hiarc, Clone, Default, Serialize, Deserialize)]
pub struct SoundSceneCreateProps {
    pub air_mode: SceneAirMode,
}

/// commands related to a sound scene
#[derive(Debug, Hiarc, Serialize, Deserialize)]
pub enum SoundCommandSoundScene {
    Create {
        id: u128,
        props: SoundSceneCreateProps,
    },
    Destroy {
        id: u128,
    },
    StayActive {
        id: u128,
    },
    StopDetatchedSounds {
        id: u128,
    },
    /// Process this scene `samples`-time.
    /// Only for offair scenes.
    ProcessOffAir {
        id: u128,
        samples: u32,
    },
}

/// commands related to a sound object
#[derive(Debug, Hiarc, Serialize, Deserialize)]
pub enum SoundCommandSoundObject {
    Create {
        id: u128,
        scene_id: u128,
        mem: SoundBackendMemory,
    },
    Destroy {
        id: u128,
        scene_id: u128,
    },
}

#[derive(Debug, Hiarc)]
pub struct StreamObjectStream(#[hiarc_skip_unsafe] pub Arc<dyn StreamDecoder>);

impl Serialize for StreamObjectStream {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        ().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for StreamObjectStream {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        <()>::deserialize(deserializer)?;
        #[derive(Debug)]
        struct DummyStream;
        impl StreamDecoder for DummyStream {
            fn sample_rate(&self) -> u32 {
                panic!("deserialization is unsupported.")
            }

            fn num_frames(&self) -> usize {
                panic!("deserialization is unsupported.")
            }

            fn decode(
                &self,
            ) -> Result<Vec<crate::stream::StreamFrame>, crate::stream::DecodeError> {
                panic!("deserialization is unsupported.")
            }

            fn seek(&self, _index: usize) -> Result<usize, anyhow::Error> {
                panic!("deserialization is unsupported.")
            }
        }
        Ok(Self(Arc::new(DummyStream)))
    }
}

/// commands related to a stream sound object
#[derive(Debug, Hiarc, Serialize, Deserialize)]
pub enum SoundCommandStreamObject {
    Create {
        id: u128,
        scene_id: u128,
        stream: StreamObjectStream,
        props: StreamPlayProps,
    },
    Destroy {
        id: u128,
        scene_id: u128,
    },
}

/// commands related to a sound listener
#[derive(Debug, Hiarc, Serialize, Deserialize)]
pub enum SoundCommandSoundListener {
    Create { id: u128, scene_id: u128, pos: vec2 },
    Update { id: u128, scene_id: u128, pos: vec2 },
    Destroy { id: u128, scene_id: u128 },
}

/// commands related to a state management
#[derive(Debug, Hiarc, Serialize, Deserialize)]
pub enum SoundCommandState {
    SoundScene(SoundCommandSoundScene),
    SoundObject(SoundCommandSoundObject),
    StreamObject(SoundCommandStreamObject),
    SoundListener(SoundCommandSoundListener),
    Swap,
}

/// commands related to actually playing/outputting sounds
#[derive(Debug, Hiarc, Serialize, Deserialize)]
pub enum SoundCommandPlay {
    Play {
        play_id: u128,
        sound_id: u128,
        scene_id: u128,
        props: SoundPlayProps,
    },
    Update {
        play_id: u128,
        sound_id: u128,
        scene_id: u128,
        props: SoundPlayBaseProps,
    },
    Pause {
        play_id: u128,
        sound_id: u128,
        scene_id: u128,
    },
    Resume {
        play_id: u128,
        sound_id: u128,
        scene_id: u128,
    },
    Stop {
        play_id: u128,
        sound_id: u128,
        scene_id: u128,
    },
    Detatch {
        play_id: u128,
        sound_id: u128,
        scene_id: u128,
    },
}

/// commands related to updating streams
#[derive(Debug, Hiarc, Serialize, Deserialize)]
pub enum SoundCommandStream {
    Update {
        stream_id: u128,
        scene_id: u128,
        props: StreamPlayBaseProps,
    },
    Pause {
        stream_id: u128,
        scene_id: u128,
    },
    Resume {
        stream_id: u128,
        scene_id: u128,
    },
}

/// collection of all commands
#[derive(Debug, Hiarc, Serialize, Deserialize)]
pub enum SoundCommand {
    State(SoundCommandState),
    Play(SoundCommandPlay),
    Stream(SoundCommandStream),
}
