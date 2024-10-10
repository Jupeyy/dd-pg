use std::rc::Rc;

use hiarc::Hiarc;

use crate::{
    backend_handle::SoundBackendHandle,
    commands::{SoundCommand, SoundCommandSoundScene, SoundCommandState, SoundSceneCreateProps},
    scene_object_shared::SceneObjectInner,
    sound_handle::SoundObjectHandle,
    sound_listener_handle::SoundListenerHandle,
    stream_handle::StreamObjectHandle,
};

/// Represents a scene, a scene must be allocated by a [`crate::scene_handle::SoundSceneHandle`]
/// from a scene you can allocate sound listeners & sound objects using the handlers.
///
/// A scene also functions as channel. So if you have background music and spartial sounds, those two
/// are simply in different scenes
#[derive(Debug, Hiarc, Clone)]
pub struct SceneObject {
    pub stream_object_handle: StreamObjectHandle,
    pub sound_object_handle: SoundObjectHandle,
    pub sound_listener_handle: SoundListenerHandle,

    // keep a ref alive
    inner: Rc<SceneObjectInner>,
}

impl SceneObject {
    pub fn new(id: u128, props: SoundSceneCreateProps, backend_handle: SoundBackendHandle) -> Self {
        backend_handle.add_cmd(SoundCommand::State(SoundCommandState::SoundScene(
            SoundCommandSoundScene::Create { id, props },
        )));
        let inner = SceneObjectInner::new(id, backend_handle.clone());

        Self {
            stream_object_handle: StreamObjectHandle::new(inner.clone()),
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

    /// Forces to stop all detatched sounds.
    pub fn stop_detatched_sounds(&self) {
        self.inner
            .backend_handle
            .add_cmd(SoundCommand::State(SoundCommandState::SoundScene(
                SoundCommandSoundScene::StopDetatchedSounds { id: self.inner.id },
            )));
    }

    /// Only call this function on off-air sound scenes.
    /// Processes the next batch of audio samples.
    pub fn process_off_air(&self, samples: u32) {
        self.inner
            .backend_handle
            .add_cmd(SoundCommand::State(SoundCommandState::SoundScene(
                SoundCommandSoundScene::ProcessOffAir {
                    id: self.inner.id,
                    samples,
                },
            )));
    }
}
