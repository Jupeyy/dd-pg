use hiarc::Hiarc;
use math::math::vector::vec2;
use serde::{Deserialize, Serialize};

use crate::{
    sound_mt_types::SoundBackendMemory,
    types::{SoundPlayBaseProps, SoundPlayProps},
};

/// commands related to a sound scene
#[derive(Debug, Hiarc, Serialize, Deserialize)]
pub enum SoundCommandSoundScene {
    Create { id: u128 },
    Destroy { id: u128 },
    StayActive { id: u128 },
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

/// collection of all commands
#[derive(Debug, Hiarc, Serialize, Deserialize)]
pub enum SoundCommand {
    State(SoundCommandState),
    Play(SoundCommandPlay),
}
