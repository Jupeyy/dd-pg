use std::borrow::Borrow;

use hiarc::Hiarc;
use map::{
    map::groups::layers::design::{Sound, SoundShape},
    skeleton::{groups::layers::design::MapLayerSoundSkeleton, resources::MapResourceRefSkeleton},
};
use math::math::{length, vector::vec2};
use sound::{sound_object::SoundObject, types::SoundPlayProps};

use super::{
    map_buffered::{ClientMapBuffered, MapSoundProcessInfo, SoundLayerSounds},
    map_with_visual::{MapVisual, MapVisualLayer},
    render_pipe::Camera,
};

#[derive(Debug, Clone, Copy)]
enum SoundLayerType {
    Background,
    Foreground,
}

/// Similar to [`crate::map::map::RenderMap`], but for map sound
#[derive(Debug, Hiarc)]
pub struct MapSoundProcess {}

impl MapSoundProcess {
    pub fn new() -> Self {
        Self {}
    }

    /// `None`, if there is no interaction
    pub fn camera_sound_interaction(pos: &vec2, sound: &Sound) -> Option<vec2> {
        let sound_pos = vec2::new(sound.pos.x.to_num(), sound.pos.y.to_num());
        let diff = *pos - sound_pos;
        match &sound.shape {
            SoundShape::Rect { size } => {
                let w: f32 = size.x.to_num();
                let h: f32 = size.y.to_num();

                let abs_x = diff.x.abs();
                let abs_y = diff.y.abs();
                if abs_x < w / 2.0 && abs_y < h / 2.0 {
                    // falloff
                    let fx = sound.falloff.to_num::<f32>() * w;
                    let fy = sound.falloff.to_num::<f32>() * h;

                    let falloff_x = if abs_x > fx {
                        (w - abs_x) / (w - fx)
                    } else {
                        1.0
                    };
                    let falloff_y = if abs_y > fy {
                        (h - abs_y) / (h - fy)
                    } else {
                        1.0
                    };

                    Some(vec2::new(falloff_x, falloff_y))
                } else {
                    None
                }
            }
            SoundShape::Circle { radius } => {
                let dist = length(&diff);

                let r = radius.to_num::<f32>();
                if dist < r {
                    let f = sound.falloff.to_num::<f32>() * r;

                    let falloff = if dist > f { (r - dist) / (r - f) } else { 1.0 };

                    Some(vec2::new(falloff, falloff))
                } else {
                    None
                }
            }
        }
    }

    pub fn handle_sound_layer<S>(
        &self,
        sounds: &Vec<MapResourceRefSkeleton<impl Borrow<SoundObject>>>,
        layer: &MapLayerSoundSkeleton<S>,
        camera: &Camera,
    ) where
        S: Borrow<SoundLayerSounds>,
    {
        if let Some(sound_index) = layer.layer.attr.sound {
            let sound_object = sounds[sound_index].user.borrow();
            for (index, sound) in layer.layer.sounds.iter().enumerate() {
                let interact = Self::camera_sound_interaction(&camera.pos, sound);
                // check if the sound should play, else play or update
                let sounds = layer.user.borrow();
                if interact.is_some() {
                    let interact = interact.unwrap();

                    if !sounds.is_playing(index) {
                        sounds.play(
                            index,
                            sound_object.play(SoundPlayProps {
                                pos: Default::default(),
                                time_offset: Default::default(),
                                looped: true,
                            }),
                        );
                    } else {
                        // update
                        // todo!();
                    }
                }
                // check if the sound is playing, but should not
                if interact.is_none() && sounds.is_playing(index) {
                    sounds.stop(index);
                }
            }
        }
    }

    fn handle_impl<'a>(
        &self,
        map: &MapVisual,
        sound_layers: impl Iterator<Item = &'a MapSoundProcessInfo>,
        layer_ty: SoundLayerType,
        camera: &Camera,
    ) {
        let groups = match layer_ty {
            SoundLayerType::Background => &map.groups.background,
            SoundLayerType::Foreground => &map.groups.foreground,
        };
        for sound_layer in sound_layers {
            let group = &groups[sound_layer.group_index];
            let MapVisualLayer::Sound(layer) = &group.layers[sound_layer.layer_index] else {
                panic!("layer was not a sound layer");
            };
            self.handle_sound_layer(&map.resources.sounds, layer, camera);
        }
    }

    pub fn handle_background(
        &self,
        map: &MapVisual,
        buffered_map: &ClientMapBuffered,
        camera: &Camera,
    ) {
        map.user.sound_scene.stay_active();
        self.handle_impl(
            map,
            buffered_map.sound.background_sound_layers.iter(),
            SoundLayerType::Background,
            camera,
        )
    }
    pub fn handle_foreground(
        &self,
        map: &MapVisual,
        buffered_map: &ClientMapBuffered,
        camera: &Camera,
    ) {
        map.user.sound_scene.stay_active();
        self.handle_impl(
            map,
            buffered_map.sound.foreground_sound_layers.iter(),
            SoundLayerType::Foreground,
            camera,
        )
    }
}
