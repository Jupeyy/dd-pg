use std::rc::Rc;

use hiarc::Hiarc;

use crate::{
    backend_handle::SoundBackendHandle,
    commands::{SoundCommand, SoundCommandSoundScene, SoundCommandState},
    scene_object_shared::SceneObjectInner,
    sound_handle::SoundObjectHandle,
    sound_listener_handle::SoundListenerHandle,
};

/// Represents a scene, a scene must be allocated by a [`crate::scene_handle::SoundSceneHandle`]
/// from a scene you can allocate sound listeners & sound objects using the handlers.
/// A scene also functions as channel. So if you have background music and spartial sounds, those two
/// are simply in different scenes
#[derive(Debug, Hiarc, Clone)]
pub struct SceneObject {
    pub sound_object_handle: SoundObjectHandle,
    pub sound_listener_handle: SoundListenerHandle,

    // keep a ref alive
    inner: Rc<SceneObjectInner>,
}

impl SceneObject {
    pub fn new(id: u128, backend_handle: SoundBackendHandle) -> Self {
        backend_handle.add_cmd(SoundCommand::State(SoundCommandState::SoundScene(
            SoundCommandSoundScene::Create { id },
        )));
        let inner = SceneObjectInner::new(id, backend_handle.clone());

        Self {
            sound_object_handle: SoundObjectHandle::new(inner.clone()),
            sound_listener_handle: SoundListenerHandle::new(inner.clone()),
            inner,
        }
    }

    /// This scene should stay active in this sound frame.
    /// It is totally safe to be called multiple times per frame.
    pub fn stay_active(&self) {
        self.inner
            .backend_handle
            .add_cmd(SoundCommand::State(SoundCommandState::SoundScene(
                SoundCommandSoundScene::StayActive { id: self.inner.id },
            )));
    }
}
