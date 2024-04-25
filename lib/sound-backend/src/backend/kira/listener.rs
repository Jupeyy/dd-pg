use std::fmt::Debug;

use hiarc::Hiarc;
use kira::spatial::{
    listener::{ListenerHandle, ListenerSettings},
    scene::SpatialSceneHandle,
};
use math::math::vector::vec2;

#[derive(Hiarc)]
pub(super) struct Listener {
    // keep for RAII
    _handle: ListenerHandle,
}

impl Debug for Listener {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Listener").finish()
    }
}

impl Listener {
    pub fn new(scene: &mut SpatialSceneHandle, pos: vec2) -> anyhow::Result<Self> {
        let handle = scene.add_listener(
            mint::Vector3 {
                x: pos.x,
                y: pos.y,
                z: 0.0,
            },
            mint::Quaternion {
                s: 1.0,
                v: mint::Vector3 {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                },
            },
            ListenerSettings::new(),
        )?;

        Ok(Self { _handle: handle })
    }
}
