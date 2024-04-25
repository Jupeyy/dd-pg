use hiarc::Hiarc;
use math::math::vector::{ffixed, fvec2, nffixed, nfvec4, ufvec2};
use serde::{Deserialize, Serialize};

use super::tiles::{MapTileLayerAttr, Tile};

#[derive(Debug, Hiarc, Clone, Serialize, Deserialize)]
pub struct MapLayerTile {
    pub attr: MapTileLayerAttr,
    pub tiles: Vec<Tile>,

    /// optional name, mostly intersting for editor
    pub name: String,
}

#[derive(Debug, Hiarc, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Quad {
    pub points: [fvec2; 5],
    pub colors: [nfvec4; 4],
    pub tex_coords: [fvec2; 4],

    pub pos_anim: Option<usize>,
    pub pos_anim_offset: time::Duration,

    pub color_anim: Option<usize>,
    pub color_anim_offset: time::Duration,
}

#[derive(Debug, Hiarc, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MapLayerQuadsAttrs {
    pub image: Option<usize>,

    /// is a high detail layer
    pub high_detail: bool,
}

#[derive(Debug, Hiarc, Clone, Serialize, Deserialize)]
pub struct MapLayerQuad {
    pub attr: MapLayerQuadsAttrs,
    pub quads: Vec<Quad>,

    /// optional name, mostly intersting for editor
    pub name: String,
}

#[derive(Debug, Hiarc, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SoundShape {
    Rect { size: ufvec2 },
    Circle { radius: ffixed },
}

#[derive(Debug, Hiarc, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sound {
    pub pos: fvec2,
    pub looped: bool,
    pub panning: bool,
    pub time_delay: time::Duration,
    pub falloff: nffixed,

    pub pos_anim: Option<usize>,
    pub pos_anim_offset: time::Duration,
    pub sound_anim: Option<usize>,
    pub sound_anim_offset: time::Duration,

    pub shape: SoundShape,
}

#[derive(Debug, Hiarc, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MapLayerSoundAttrs {
    pub sound: Option<usize>,

    /// is a high detail layer
    pub high_detail: bool,
}

#[derive(Debug, Hiarc, Clone, Serialize, Deserialize)]
pub struct MapLayerSound {
    pub attr: MapLayerSoundAttrs,
    pub sounds: Vec<Sound>,

    /// optional name, mostly intersting for editor
    pub name: String,
}

#[derive(Debug, Hiarc, Clone, Serialize, Deserialize)]
pub enum MapLayer {
    /// can be used for mods, if client compability is important, while having custom layers
    Abritrary(Vec<u8>),
    Tile(MapLayerTile),
    Quad(MapLayerQuad),
    Sound(MapLayerSound),
}

impl MapLayer {
    pub fn name(&self) -> &str {
        match self {
            MapLayer::Abritrary(_) => "unknown layer",
            MapLayer::Tile(layer) => &layer.name,
            MapLayer::Quad(layer) => &layer.name,
            MapLayer::Sound(layer) => &layer.name,
        }
    }
}
