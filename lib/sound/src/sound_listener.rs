use std::rc::Rc;

use hiarc::{hiarc_safer_rc_refcell, Hiarc};
use math::math::vector::vec2;

use crate::{
    commands::{SoundCommand, SoundCommandSoundListener, SoundCommandState},
    scene_object_shared::SceneObjectInner,
};

/// a sound listener is an object that listens on all sounds in a scene, it is allocated
/// by the sound listener handle ([`crate::sound_listener_handle::SoundListenerHandle`])
#[hiarc_safer_rc_refcell]
#[derive(Debug, Hiarc)]
pub struct SoundListener {
    id: u128,

    // must outlive the scene
    scene: Rc<SceneObjectInner>,

    forgotten: bool,
}

#[hiarc_safer_rc_refcell]
impl SoundListener {
    pub fn new(id: u128, pos: vec2, scene: Rc<SceneObjectInner>) -> Self {
        scene
            .backend_handle
            .add_cmd(SoundCommand::State(SoundCommandState::SoundListener(
                SoundCommandSoundListener::Create {
                    id,
                    scene_id: scene.id,
                    pos,
                },
            )));

        Self {
            id,
            scene,
            forgotten: false,
        }
    }

    /// Forget about this listener. it will be cleaned up automatically if the scene is cleaned up.
    /// This is usually only useful if sounds are relative to the listener (e.g. background music).
    ///
    /// ## Note
    /// #### If you keep this listener instance alive you can still update the position etc.
    /// #### dropping it will simply not remove it from the scene
    pub fn forget(&mut self) {
        self.forgotten = true;
    }
}

#[hiarc_safer_rc_refcell]
impl Drop for SoundListener {
    fn drop(&mut self) {
        if !self.forgotten {
            self.scene.backend_handle.add_cmd(SoundCommand::State(
                SoundCommandState::SoundListener(SoundCommandSoundListener::Destroy {
                    id: self.id,
                    scene_id: self.scene.id,
                }),
            ));
        }
    }
}
