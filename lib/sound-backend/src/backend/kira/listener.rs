use std::fmt::Debug;

use hiarc::Hiarc;
use kira::spatial::{
    listener::{ListenerHandle, ListenerSettings},
    scene::SpatialSceneHandle,
};
use math::math::vector::vec2;

use super::instance::Instance;

#[derive(Hiarc)]
pub(super) struct Listener {
    // keep for RAII
    handle: ListenerHandle,
}

impl Debug for Listener {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Listener").finish()
    }
}

impl Listener {
    pub fn new(
        instance: &mut Instance,
        scene: &mut SpatialSceneHandle,
        pos: vec2,
    ) -> anyhow::Result<Self> {
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
            ListenerSettings::new().track(instance.track()),
        )?;

        Ok(Self { handle })
    }

    pub fn update(&mut self, pos: vec2) {
        self.handle.set_position(
            mint::Vector3 {
                x: pos.x,
                y: pos.y,
                z: 0.0,
            },
            Default::default(),
        );
    }
}
