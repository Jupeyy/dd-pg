use std::rc::Rc;

use hiarc::Hiarc;

use crate::{
    commands::{SoundCommand, SoundCommandSoundObject, SoundCommandState},
    scene_object_shared::SceneObjectInner,
};

#[derive(Debug, Hiarc)]
pub struct SoundObjectInner {
    pub(crate) id: u128,

    // must outlive the scene
    pub(crate) scene: Rc<SceneObjectInner>,
}

impl SoundObjectInner {
    pub fn new(id: u128, scene: Rc<SceneObjectInner>) -> Rc<Self> {
        Rc::new(Self { id, scene })
    }
}

impl Drop for SoundObjectInner {
    fn drop(&mut self) {
        self.scene
            .backend_handle
            .add_cmd(SoundCommand::State(SoundCommandState::SoundObject(
                SoundCommandSoundObject::Destroy {
                    id: self.id,
                    scene_id: self.scene.id,
                },
            )));
    }
}
