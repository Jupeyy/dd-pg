use hiarc::{hiarc_safer_rc_refcell, Hiarc};

use crate::{backend_handle::SoundBackendHandle, scene_object::SceneObject};

/// allocates new scene objects
#[hiarc_safer_rc_refcell]
#[derive(Debug, Hiarc)]
pub struct SoundSceneHandle {
    backend_handle: SoundBackendHandle,
    id_gen: u128,
}

#[hiarc_safer_rc_refcell]
impl SoundSceneHandle {
    pub fn new(backend_handle: SoundBackendHandle) -> Self {
        Self {
            backend_handle,
            id_gen: 0,
        }
    }

    pub fn create(&mut self) -> SceneObject {
        let id = self.id_gen;
        self.id_gen += 1;

        SceneObject::new(id, self.backend_handle.clone())
    }
}
