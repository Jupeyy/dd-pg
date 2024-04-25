use std::rc::Rc;

use hiarc::Hiarc;

use crate::{
    commands::{SoundCommand, SoundCommandPlay},
    sound_object_shared::SoundObjectInner,
    types::SoundPlayProps,
};

/// handles the playback of a currenly played sound
/// if dropped and [`SoundPlayHandle::detatch`] was not previously
/// called, it will automatically stop the play.
#[derive(Debug, Hiarc)]
pub struct SoundPlayHandle {
    play_id: u128,

    sound_object: Rc<SoundObjectInner>,

    detatched: bool,
}

impl SoundPlayHandle {
    pub fn new(play_id: u128, sound_object: Rc<SoundObjectInner>, props: SoundPlayProps) -> Self {
        sound_object
            .scene
            .backend_handle
            .add_cmd(SoundCommand::Play(SoundCommandPlay::Play {
                play_id,
                sound_id: sound_object.id,
                scene_id: sound_object.scene.id,
                props,
            }));

        Self {
            play_id,

            sound_object,

            detatched: false,
        }
    }

    /// detatches the play handle, which means that it will keep the sound alive until it stopped
    pub fn detatch(mut self) {
        self.detatched = true;
    }

    /// stop playing this sound
    pub fn stop(self) {
        // nothing to do
    }
}

impl Drop for SoundPlayHandle {
    fn drop(&mut self) {
        if !self.detatched {
            self.sound_object
                .scene
                .backend_handle
                .add_cmd(SoundCommand::Play(SoundCommandPlay::Stop {
                    play_id: self.play_id,
                    sound_id: self.sound_object.id,
                    scene_id: self.sound_object.scene.id,
                }))
        } else {
            self.sound_object
                .scene
                .backend_handle
                .add_cmd(SoundCommand::Play(SoundCommandPlay::Detatch {
                    play_id: self.play_id,
                    sound_id: self.sound_object.id,
                    scene_id: self.sound_object.scene.id,
                }))
        }
    }
}
