use std::rc::Rc;

use hiarc::Hiarc;

use crate::{
    backend_handle::SoundBackendHandle,
    commands::{SoundCommand, SoundCommandSoundScene, SoundCommandState},
};

#[derive(Debug, Hiarc)]
pub struct SceneObjectInner {
    pub(crate) id: u128,
    pub(crate) backend_handle: SoundBackendHandle,
}

impl SceneObjectInner {
    pub fn new(id: u128, backend_handle: SoundBackendHandle) -> Rc<Self> {
        Rc::new(Self { backend_handle, id })
    }
}

impl Drop for SceneObjectInner {
    fn drop(&mut self) {
        self.backend_handle
            .add_cmd(SoundCommand::State(SoundCommandState::SoundScene(
                SoundCommandSoundScene::Destroy { id: self.id },
            )));
    }
}
