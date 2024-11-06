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
    pub(super) handle: Option<ListenerHandle>,

    pos: mint::Vector3<f32>,
}

impl Debug for Listener {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Listener").finish()
    }
}

impl Listener {
    pub fn new(instance: &mut Instance, scene: Option<&mut SpatialSceneHandle>, pos: vec2) -> Self {
        let pos = mint::Vector3 {
            x: pos.x,
            y: pos.y,
            z: 0.0,
        };

        let handle = scene.and_then(|scene| Self::listener_impl(scene, instance, pos).ok());

        Self { handle, pos }
    }

    fn listener_impl(
        scene: &mut SpatialSceneHandle,
        instance: &Instance,
        pos: mint::Vector3<f32>,
    ) -> anyhow::Result<ListenerHandle> {
        Ok(scene.add_listener(
            pos,
            mint::Quaternion {
                s: 1.0,
                v: mint::Vector3 {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                },
            },
            ListenerSettings::new().track(instance.track()),
        )?)
    }

    pub fn update(&mut self, pos: vec2) {
        let pos = mint::Vector3 {
            x: pos.x,
            y: pos.y,
            z: 0.0,
        };
        if let Some(handle) = &mut self.handle {
            handle.set_position(pos, Default::default());
        }
    }

    pub fn reattach_to_scene(&mut self, scene: &mut SpatialSceneHandle, instance: &Instance) {
        self.handle = Listener::listener_impl(scene, instance, self.pos).ok();
    }
}
