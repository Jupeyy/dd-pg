use hashlink::LinkedHashMap;
use map::{
    map::{
        animations::{ColorAnimation, PosAnimation, SoundAnimation},
        groups::{
            layers::{
                design::{
                    MapLayerQuad, MapLayerQuadsAttrs, MapLayerSound, MapLayerSoundAttrs,
                    MapLayerTile, Quad, Sound,
                },
                physics::MapLayerPhysics,
                tiles::{MapTileLayerAttr, MapTileLayerPhysicsTiles, Tile},
            },
            MapGroup, MapGroupAttr, MapGroupPhysicsAttr,
        },
        resources::MapResourceRef,
    },
    types::NonZeroU16MinusOne,
};
use serde::{Deserialize, Serialize};

/// an action that results in a change in the state of the map
/// this action is usually shared across all clients
/// additionally every action must be able to handle the undo to that action
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EditorAction {
    // gui swaps
    SwapGroups(ActSwapGroups),
    SwapLayers(ActSwapLayers),
    // add image/sound
    AddImage(ActAddImage),
    AddImage2dArray(ActAddImage2dArray),
    AddSound(ActAddSound),
    RemImage(ActRemImage),
    RemImage2dArray(ActRemImage2dArray),
    RemSound(ActRemSound),
    // change image/sound indices
    LayerChangeImageIndex(ActLayerChangeImageIndex),
    LayerChangeSoundIndex(ActLayerChangeSoundIndex),
    // rem/add quads/sounds
    QuadLayerAddQuads(ActQuadLayerAddQuads),
    SoundLayerAddSounds(ActSoundLayerAddSounds),
    QuadLayerRemQuads(ActQuadLayerRemQuads),
    SoundLayerRemSounds(ActSoundLayerRemSounds),
    // rem/add layers
    AddTileLayer(ActAddTileLayer),
    AddQuadLayer(ActAddQuadLayer),
    AddSoundLayer(ActAddSoundLayer),
    RemTileLayer(ActRemTileLayer),
    RemQuadLayer(ActRemQuadLayer),
    RemSoundLayer(ActRemSoundLayer),
    // rem/add physics layer
    AddPhysicsTileLayer(ActAddPhysicsTileLayer),
    RemPhysicsTileLayer(ActRemPhysicsTileLayer),
    // rem/add tiles
    TileLayerReplaceTiles(ActTileLayerReplaceTiles),
    TilePhysicsLayerReplaceTiles(ActTilePhysicsLayerReplaceTiles),
    // rem/add group
    AddGroup(ActAddGroup),
    RemGroup(ActRemGroup),
    // change attributes
    ChangeGroupAttr(ActChangeGroupAttr),
    ChangePhysicsGroupAttr(ActChangePhysicsGroupAttr),
    ChangeTileLayerDesignAttr(ActChangeTileLayerDesignAttr),
    ChangeQuadLayerAttr(ActChangeQuadLayerAttr),
    ChangeSoundLayerAttr(ActChangeSoundLayerAttr),
    ChangeQuadAttr(Box<ActChangeQuadAttr>),
    ChangeSoundAttr(ActChangeSoundAttr),
    ChangeTeleporter(ActChangeTeleporter),
    ChangeSwitch(ActChangeSwitch),
    ChangeTuneZone(ActChangeTuneZone),
    // add/rem animations
    AddPosAnim(ActAddPosAnim),
    RemPosAnim(ActRemPosAnim),
    AddColorAnim(ActAddColorAnim),
    RemColorAnim(ActRemColorAnim),
    AddSoundAnim(ActAddSoundAnim),
    RemSoundAnim(ActRemSoundAnim),
}

/// actions are always grouped, even single actions
/// action groups are there to make it easier to undo e.g. brushes
/// instead of undoing brush paints one by one, the whole group of action is undone at once
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditorActionGroup {
    pub actions: Vec<EditorAction>,

    /// the identifier can be optionally set to know if the current group contains
    /// similar actions, e.g. when painting with a brush the brush can now identify
    /// if this group fits its needs
    /// a value of `None` indicates that this action should never be grouped
    pub identifier: Option<String>,
}

pub trait EditorActionInterface {
    fn undo_info(&self) -> String;
    fn redo_info(&self) -> String;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActSwapGroups {
    pub is_background: bool,
    pub group1: usize,
    pub group2: usize,
}

impl EditorActionInterface for ActSwapGroups {
    fn undo_info(&self) -> String {
        format!(
            "Swapped group #{} & #{} in {}",
            self.group1,
            self.group2,
            if self.is_background {
                "background"
            } else {
                "foreground"
            }
        )
    }

    fn redo_info(&self) -> String {
        self.undo_info()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActSwapLayers {
    pub is_background: bool,
    pub layer1: usize,
    pub layer2: usize,
    pub group: usize,
}

impl EditorActionInterface for ActSwapLayers {
    fn undo_info(&self) -> String {
        format!(
            "Swapped layer #{} and #{} of group #{} in {}",
            self.layer1,
            self.layer2,
            self.group,
            if self.is_background {
                "background"
            } else {
                "foreground"
            }
        )
    }

    fn redo_info(&self) -> String {
        self.undo_info()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActAddRemImage {
    pub res: MapResourceRef,
    pub file: Vec<u8>,

    pub index: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActAddRemSound {
    pub res: MapResourceRef,
    pub file: Vec<u8>,

    pub index: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActAddImage {
    pub base: ActAddRemImage,
}

impl EditorActionInterface for ActAddImage {
    fn undo_info(&self) -> String {
        format!("Remove image \"{}\"", self.base.res.name.as_str())
    }

    fn redo_info(&self) -> String {
        format!("Add image \"{}\"", self.base.res.name.as_str())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActAddImage2dArray {
    pub base: ActAddRemImage,
}

impl EditorActionInterface for ActAddImage2dArray {
    fn undo_info(&self) -> String {
        format!("Remove image 2d array \"{}\"", self.base.res.name.as_str())
    }

    fn redo_info(&self) -> String {
        format!("Add image 2d array \"{}\"", self.base.res.name.as_str())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActAddSound {
    pub base: ActAddRemSound,
}

impl EditorActionInterface for ActAddSound {
    fn undo_info(&self) -> String {
        format!("Remove sound \"{}\"", self.base.res.name.as_str())
    }

    fn redo_info(&self) -> String {
        format!("Add sound \"{}\"", self.base.res.name.as_str())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActRemImage {
    pub base: ActAddRemImage,
}

impl EditorActionInterface for ActRemImage {
    fn undo_info(&self) -> String {
        format!("Add image \"{}\"", self.base.res.name.as_str())
    }

    fn redo_info(&self) -> String {
        format!("Remove image \"{}\"", self.base.res.name.as_str())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActRemImage2dArray {
    pub base: ActAddRemImage,
}

impl EditorActionInterface for ActRemImage2dArray {
    fn undo_info(&self) -> String {
        format!("Add image 2d array \"{}\"", self.base.res.name.as_str())
    }

    fn redo_info(&self) -> String {
        format!("Remove image 2d array \"{}\"", self.base.res.name.as_str())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActRemSound {
    pub base: ActAddRemSound,
}

impl EditorActionInterface for ActRemSound {
    fn undo_info(&self) -> String {
        format!("Add sound \"{}\"", self.base.res.name.as_str())
    }

    fn redo_info(&self) -> String {
        format!("Remove sound \"{}\"", self.base.res.name.as_str())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActLayerChangeImageIndex {
    pub is_background: bool,
    pub group_index: usize,
    pub layer_index: usize,

    pub old_index: Option<usize>,
    pub new_index: Option<usize>,
}

impl EditorActionInterface for ActLayerChangeImageIndex {
    fn undo_info(&self) -> String {
        format!(
            "Change layer #{}'s image index from {:?} to {:?} in {}",
            self.layer_index,
            self.new_index,
            self.old_index,
            if self.is_background {
                "background"
            } else {
                "foreground"
            }
        )
    }

    fn redo_info(&self) -> String {
        format!(
            "Change layer #{}'s image index from {:?} to {:?} in {}",
            self.layer_index,
            self.old_index,
            self.new_index,
            if self.is_background {
                "background"
            } else {
                "foreground"
            }
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActLayerChangeSoundIndex {
    pub is_background: bool,
    pub group_index: usize,
    pub layer_index: usize,

    pub old_index: Option<usize>,
    pub new_index: Option<usize>,
}

impl EditorActionInterface for ActLayerChangeSoundIndex {
    fn undo_info(&self) -> String {
        format!(
            "Change layer #{}'s sound index from {:?} to {:?} in {}",
            self.layer_index,
            self.new_index,
            self.old_index,
            if self.is_background {
                "background"
            } else {
                "foreground"
            }
        )
    }

    fn redo_info(&self) -> String {
        format!(
            "Change layer #{}'s sound index from {:?} to {:?} in {}",
            self.layer_index,
            self.old_index,
            self.new_index,
            if self.is_background {
                "background"
            } else {
                "foreground"
            }
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActQuadLayerAddRemQuads {
    pub is_background: bool,
    pub group_index: usize,
    pub layer_index: usize,

    pub index: usize,
    pub quads: Vec<Quad>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActSoundLayerAddRemSounds {
    pub is_background: bool,
    pub group_index: usize,
    pub layer_index: usize,

    pub index: usize,
    pub sounds: Vec<Sound>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActQuadLayerAddQuads {
    pub base: ActQuadLayerAddRemQuads,
}

impl EditorActionInterface for ActQuadLayerAddQuads {
    fn undo_info(&self) -> String {
        format!(
            "Remove {} quads @{} from layer #{} in {}",
            self.base.quads.len(),
            self.base.index,
            self.base.layer_index,
            if self.base.is_background {
                "background"
            } else {
                "foreground"
            }
        )
    }

    fn redo_info(&self) -> String {
        format!(
            "Add {} quads @{} to layer #{} in {}",
            self.base.quads.len(),
            self.base.index,
            self.base.layer_index,
            if self.base.is_background {
                "background"
            } else {
                "foreground"
            }
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActSoundLayerAddSounds {
    pub base: ActSoundLayerAddRemSounds,
}

impl EditorActionInterface for ActSoundLayerAddSounds {
    fn undo_info(&self) -> String {
        format!(
            "Remove {} sounds @{} from layer #{} in {}",
            self.base.sounds.len(),
            self.base.index,
            self.base.layer_index,
            if self.base.is_background {
                "background"
            } else {
                "foreground"
            }
        )
    }

    fn redo_info(&self) -> String {
        format!(
            "Add {} sounds @{} to layer #{} in {}",
            self.base.sounds.len(),
            self.base.index,
            self.base.layer_index,
            if self.base.is_background {
                "background"
            } else {
                "foreground"
            }
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActQuadLayerRemQuads {
    pub base: ActQuadLayerAddRemQuads,
}

impl EditorActionInterface for ActQuadLayerRemQuads {
    fn undo_info(&self) -> String {
        format!(
            "Add {} quads @{} to layer #{} in {}",
            self.base.quads.len(),
            self.base.index,
            self.base.layer_index,
            if self.base.is_background {
                "background"
            } else {
                "foreground"
            }
        )
    }

    fn redo_info(&self) -> String {
        format!(
            "Remove {} quads @{} from layer #{} in {}",
            self.base.quads.len(),
            self.base.index,
            self.base.layer_index,
            if self.base.is_background {
                "background"
            } else {
                "foreground"
            }
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActSoundLayerRemSounds {
    pub base: ActSoundLayerAddRemSounds,
}

impl EditorActionInterface for ActSoundLayerRemSounds {
    fn undo_info(&self) -> String {
        format!(
            "Add {} sounds @{} to layer #{} in {}",
            self.base.sounds.len(),
            self.base.index,
            self.base.layer_index,
            if self.base.is_background {
                "background"
            } else {
                "foreground"
            }
        )
    }

    fn redo_info(&self) -> String {
        format!(
            "Remove {} sounds @{} from layer #{} in {}",
            self.base.sounds.len(),
            self.base.index,
            self.base.layer_index,
            if self.base.is_background {
                "background"
            } else {
                "foreground"
            }
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActAddRemTileLayer {
    pub is_background: bool,
    pub group_index: usize,
    pub index: usize,
    pub layer: MapLayerTile,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActAddRemQuadLayer {
    pub is_background: bool,
    pub group_index: usize,
    pub index: usize,
    pub layer: MapLayerQuad,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActAddRemSoundLayer {
    pub is_background: bool,
    pub group_index: usize,
    pub index: usize,
    pub layer: MapLayerSound,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActAddTileLayer {
    pub base: ActAddRemTileLayer,
}

impl EditorActionInterface for ActAddTileLayer {
    fn undo_info(&self) -> String {
        if !self.base.layer.name.is_empty() {
            format!(
                "Remove tile layer \"{}\" #{} from group #{} in {}",
                self.base.layer.name,
                self.base.index,
                self.base.group_index,
                if self.base.is_background {
                    "background"
                } else {
                    "foreground"
                }
            )
        } else {
            format!(
                "Remove tile layer #{} from group #{} in {}",
                self.base.index,
                self.base.group_index,
                if self.base.is_background {
                    "background"
                } else {
                    "foreground"
                }
            )
        }
    }

    fn redo_info(&self) -> String {
        if !self.base.layer.name.is_empty() {
            format!(
                "Add tile layer \"{}\" @{} to group #{} in {}",
                self.base.layer.name,
                self.base.index,
                self.base.group_index,
                if self.base.is_background {
                    "background"
                } else {
                    "foreground"
                }
            )
        } else {
            format!(
                "Add tile layer @{} to group #{} in {}",
                self.base.index,
                self.base.group_index,
                if self.base.is_background {
                    "background"
                } else {
                    "foreground"
                }
            )
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActRemTileLayer {
    pub base: ActAddRemTileLayer,
}

impl EditorActionInterface for ActRemTileLayer {
    fn undo_info(&self) -> String {
        if !self.base.layer.name.is_empty() {
            format!(
                "Add tile layer \"{}\" @{} to group #{} in {}",
                self.base.layer.name,
                self.base.index,
                self.base.group_index,
                if self.base.is_background {
                    "background"
                } else {
                    "foreground"
                }
            )
        } else {
            format!(
                "Add tile layer @{} to group #{} in {}",
                self.base.index,
                self.base.group_index,
                if self.base.is_background {
                    "background"
                } else {
                    "foreground"
                }
            )
        }
    }

    fn redo_info(&self) -> String {
        if !self.base.layer.name.is_empty() {
            format!(
                "Remove tile layer \"{}\" #{} from group #{} in {}",
                self.base.layer.name,
                self.base.index,
                self.base.group_index,
                if self.base.is_background {
                    "background"
                } else {
                    "foreground"
                }
            )
        } else {
            format!(
                "Remove tile layer #{} from group #{} in {}",
                self.base.index,
                self.base.group_index,
                if self.base.is_background {
                    "background"
                } else {
                    "foreground"
                }
            )
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActAddQuadLayer {
    pub base: ActAddRemQuadLayer,
}

impl EditorActionInterface for ActAddQuadLayer {
    fn undo_info(&self) -> String {
        if !self.base.layer.name.is_empty() {
            format!(
                "Remove quad layer \"{}\" #{} from group #{} in {}",
                self.base.layer.name,
                self.base.index,
                self.base.group_index,
                if self.base.is_background {
                    "background"
                } else {
                    "foreground"
                }
            )
        } else {
            format!(
                "Remove quad layer #{} from group #{} in {}",
                self.base.index,
                self.base.group_index,
                if self.base.is_background {
                    "background"
                } else {
                    "foreground"
                }
            )
        }
    }

    fn redo_info(&self) -> String {
        if !self.base.layer.name.is_empty() {
            format!(
                "Add quad layer \"{}\" @{} to group #{} in {}",
                self.base.layer.name,
                self.base.index,
                self.base.group_index,
                if self.base.is_background {
                    "background"
                } else {
                    "foreground"
                }
            )
        } else {
            format!(
                "Add quad layer @{} to group #{} in {}",
                self.base.index,
                self.base.group_index,
                if self.base.is_background {
                    "background"
                } else {
                    "foreground"
                }
            )
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActRemQuadLayer {
    pub base: ActAddRemQuadLayer,
}

impl EditorActionInterface for ActRemQuadLayer {
    fn undo_info(&self) -> String {
        if !self.base.layer.name.is_empty() {
            format!(
                "Add quad layer \"{}\" @{} to group #{} in {}",
                self.base.layer.name,
                self.base.index,
                self.base.group_index,
                if self.base.is_background {
                    "background"
                } else {
                    "foreground"
                }
            )
        } else {
            format!(
                "Add quad layer @{} to group #{} in {}",
                self.base.index,
                self.base.group_index,
                if self.base.is_background {
                    "background"
                } else {
                    "foreground"
                }
            )
        }
    }

    fn redo_info(&self) -> String {
        if !self.base.layer.name.is_empty() {
            format!(
                "Remove quad layer \"{}\" #{} from group #{} in {}",
                self.base.layer.name,
                self.base.index,
                self.base.group_index,
                if self.base.is_background {
                    "background"
                } else {
                    "foreground"
                }
            )
        } else {
            format!(
                "Remove quad layer #{} from group #{} in {}",
                self.base.index,
                self.base.group_index,
                if self.base.is_background {
                    "background"
                } else {
                    "foreground"
                }
            )
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActAddSoundLayer {
    pub base: ActAddRemSoundLayer,
}

impl EditorActionInterface for ActAddSoundLayer {
    fn undo_info(&self) -> String {
        if !self.base.layer.name.is_empty() {
            format!(
                "Remove sound layer \"{}\" #{} from group #{} in {}",
                self.base.layer.name,
                self.base.index,
                self.base.group_index,
                if self.base.is_background {
                    "background"
                } else {
                    "foreground"
                }
            )
        } else {
            format!(
                "Remove sound layer #{} from group #{} in {}",
                self.base.index,
                self.base.group_index,
                if self.base.is_background {
                    "background"
                } else {
                    "foreground"
                }
            )
        }
    }

    fn redo_info(&self) -> String {
        if !self.base.layer.name.is_empty() {
            format!(
                "Add sound layer \"{}\" @{} to group #{} in {}",
                self.base.layer.name,
                self.base.index,
                self.base.group_index,
                if self.base.is_background {
                    "background"
                } else {
                    "foreground"
                }
            )
        } else {
            format!(
                "Add sound layer @{} to group #{} in {}",
                self.base.index,
                self.base.group_index,
                if self.base.is_background {
                    "background"
                } else {
                    "foreground"
                }
            )
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActRemSoundLayer {
    pub base: ActAddRemSoundLayer,
}

impl EditorActionInterface for ActRemSoundLayer {
    fn undo_info(&self) -> String {
        if !self.base.layer.name.is_empty() {
            format!(
                "Add sound layer \"{}\" @{} to group #{} in {}",
                self.base.layer.name,
                self.base.index,
                self.base.group_index,
                if self.base.is_background {
                    "background"
                } else {
                    "foreground"
                }
            )
        } else {
            format!(
                "Add sound layer @{} to group #{} in {}",
                self.base.index,
                self.base.group_index,
                if self.base.is_background {
                    "background"
                } else {
                    "foreground"
                }
            )
        }
    }

    fn redo_info(&self) -> String {
        if !self.base.layer.name.is_empty() {
            format!(
                "Remove sound layer \"{}\" #{} from group #{} in {}",
                self.base.layer.name,
                self.base.index,
                self.base.group_index,
                if self.base.is_background {
                    "background"
                } else {
                    "foreground"
                }
            )
        } else {
            format!(
                "Remove sound layer #{} from group #{} in {}",
                self.base.index,
                self.base.group_index,
                if self.base.is_background {
                    "background"
                } else {
                    "foreground"
                }
            )
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActAddRemPhysicsTileLayer {
    pub index: usize,
    pub layer: MapLayerPhysics,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActAddPhysicsTileLayer {
    pub base: ActAddRemPhysicsTileLayer,
}

fn layer_name_phy(layer: &MapLayerPhysics) -> &str {
    match layer {
        MapLayerPhysics::Arbitrary(_) => "Arbitrary",
        MapLayerPhysics::Game(_) => "Game",
        MapLayerPhysics::Front(_) => "Front",
        MapLayerPhysics::Tele(_) => "Tele",
        MapLayerPhysics::Speedup(_) => "Speedup",
        MapLayerPhysics::Switch(_) => "Switch",
        MapLayerPhysics::Tune(_) => "Tune",
    }
}

impl EditorActionInterface for ActAddPhysicsTileLayer {
    fn undo_info(&self) -> String {
        format!(
            "Remove physics layer \"{}\" @{}",
            layer_name_phy(&self.base.layer),
            self.base.index,
        )
    }

    fn redo_info(&self) -> String {
        format!(
            "Add physics layer \"{}\" @{}",
            layer_name_phy(&self.base.layer),
            self.base.index
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActRemPhysicsTileLayer {
    pub base: ActAddRemPhysicsTileLayer,
}

impl EditorActionInterface for ActRemPhysicsTileLayer {
    fn undo_info(&self) -> String {
        format!(
            "Add physics layer \"{}\" @{}",
            layer_name_phy(&self.base.layer),
            self.base.index,
        )
    }

    fn redo_info(&self) -> String {
        format!(
            "Remove physics layer \"{}\" @{}",
            layer_name_phy(&self.base.layer),
            self.base.index,
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActTileLayerReplTilesBase {
    pub is_background: bool,
    pub group_index: usize,
    pub layer_index: usize,

    pub old_tiles: Vec<Tile>,
    pub new_tiles: Vec<Tile>,
    pub x: u16,
    pub y: u16,
    pub w: NonZeroU16MinusOne,
    pub h: NonZeroU16MinusOne,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActTileLayerReplaceTiles {
    pub base: ActTileLayerReplTilesBase,
}

impl EditorActionInterface for ActTileLayerReplaceTiles {
    fn undo_info(&self) -> String {
        format!(
            "Replace {} tiles with {} tiles @({}, {})-({}, {}) from layer #{} in {}",
            self.base.new_tiles.iter().filter(|t| t.index > 0).count(),
            self.base.old_tiles.iter().filter(|t| t.index > 0).count(),
            self.base.x,
            self.base.y,
            self.base.x + self.base.w.get(),
            self.base.y + self.base.h.get(),
            self.base.layer_index,
            if self.base.is_background {
                "background"
            } else {
                "foreground"
            }
        )
    }

    fn redo_info(&self) -> String {
        format!(
            "Replace {} tiles with {} tiles @({}, {})-({}, {}) to layer #{} in {}",
            self.base.old_tiles.iter().filter(|t| t.index > 0).count(),
            self.base.new_tiles.iter().filter(|t| t.index > 0).count(),
            self.base.x,
            self.base.y,
            self.base.x + self.base.w.get(),
            self.base.y + self.base.h.get(),
            self.base.layer_index,
            if self.base.is_background {
                "background"
            } else {
                "foreground"
            }
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActTilePhysicsLayerReplTilesBase {
    pub layer_index: usize,

    pub old_tiles: MapTileLayerPhysicsTiles,
    pub new_tiles: MapTileLayerPhysicsTiles,
    pub x: u16,
    pub y: u16,
    pub w: NonZeroU16MinusOne,
    pub h: NonZeroU16MinusOne,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActTilePhysicsLayerReplaceTiles {
    pub base: ActTilePhysicsLayerReplTilesBase,
}

impl EditorActionInterface for ActTilePhysicsLayerReplaceTiles {
    fn undo_info(&self) -> String {
        format!(
            "Replace {} tiles with {} tiles @({}, {})-({}, {}) from layer #{}",
            self.base.new_tiles.non_air_tiles_count(),
            self.base.old_tiles.non_air_tiles_count(),
            self.base.x,
            self.base.y,
            self.base.x + self.base.w.get(),
            self.base.y + self.base.h.get(),
            self.base.layer_index,
        )
    }

    fn redo_info(&self) -> String {
        format!(
            "Replace {} tiles with {} tiles @({}, {})-({}, {}) to layer #{}",
            self.base.old_tiles.non_air_tiles_count(),
            self.base.new_tiles.non_air_tiles_count(),
            self.base.x,
            self.base.y,
            self.base.x + self.base.w.get(),
            self.base.y + self.base.h.get(),
            self.base.layer_index,
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActAddRemGroup {
    pub is_background: bool,
    pub index: usize,
    pub group: MapGroup,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActAddGroup {
    pub base: ActAddRemGroup,
}

impl EditorActionInterface for ActAddGroup {
    fn undo_info(&self) -> String {
        if !self.base.group.name.is_empty() {
            format!(
                "Remove group \"{}\" @{} in {}",
                self.base.group.name,
                self.base.index,
                if self.base.is_background {
                    "background"
                } else {
                    "foreground"
                }
            )
        } else {
            format!(
                "Remove group @{} in {}",
                self.base.index,
                if self.base.is_background {
                    "background"
                } else {
                    "foreground"
                }
            )
        }
    }

    fn redo_info(&self) -> String {
        if !self.base.group.name.is_empty() {
            format!(
                "Add group \"{}\" @{} in {}",
                self.base.group.name,
                self.base.index,
                if self.base.is_background {
                    "background"
                } else {
                    "foreground"
                }
            )
        } else {
            format!(
                "Add group @{} in {}",
                self.base.index,
                if self.base.is_background {
                    "background"
                } else {
                    "foreground"
                }
            )
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActRemGroup {
    pub base: ActAddRemGroup,
}

impl EditorActionInterface for ActRemGroup {
    fn undo_info(&self) -> String {
        if !self.base.group.name.is_empty() {
            format!(
                "Add group \"{}\" @{} in {}",
                self.base.group.name,
                self.base.index,
                if self.base.is_background {
                    "background"
                } else {
                    "foreground"
                }
            )
        } else {
            format!(
                "Add group @{} in {}",
                self.base.index,
                if self.base.is_background {
                    "background"
                } else {
                    "foreground"
                }
            )
        }
    }

    fn redo_info(&self) -> String {
        if !self.base.group.name.is_empty() {
            format!(
                "Remove group \"{}\" @{} in {}",
                self.base.group.name,
                self.base.index,
                if self.base.is_background {
                    "background"
                } else {
                    "foreground"
                }
            )
        } else {
            format!(
                "Remove group @{} in {}",
                self.base.index,
                if self.base.is_background {
                    "background"
                } else {
                    "foreground"
                }
            )
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActChangeGroupAttr {
    pub is_background: bool,
    pub group_index: usize,
    pub old_attr: MapGroupAttr,
    pub new_attr: MapGroupAttr,
}

impl EditorActionInterface for ActChangeGroupAttr {
    fn undo_info(&self) -> String {
        format!(
            "Change attributes of group #{} in {}",
            self.group_index,
            if self.is_background {
                "background"
            } else {
                "foreground"
            }
        )
    }

    fn redo_info(&self) -> String {
        format!(
            "Change attributes of group #{} in {}",
            self.group_index,
            if self.is_background {
                "background"
            } else {
                "foreground"
            }
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActChangePhysicsGroupAttr {
    pub old_attr: MapGroupPhysicsAttr,
    pub new_attr: MapGroupPhysicsAttr,

    pub old_layer_tiles: Vec<MapTileLayerPhysicsTiles>,
    pub new_layer_tiles: Vec<MapTileLayerPhysicsTiles>,
}

impl EditorActionInterface for ActChangePhysicsGroupAttr {
    fn undo_info(&self) -> String {
        "Change attributes of physics group".to_string()
    }

    fn redo_info(&self) -> String {
        "Change attributes of physics group".to_string()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActChangeTileLayerDesignAttr {
    pub is_background: bool,
    pub group_index: usize,
    pub layer_index: usize,
    pub old_attr: MapTileLayerAttr,
    pub new_attr: MapTileLayerAttr,

    pub old_tiles: Vec<Tile>,
    pub new_tiles: Vec<Tile>,
}

impl EditorActionInterface for ActChangeTileLayerDesignAttr {
    fn undo_info(&self) -> String {
        format!(
            "Change attributes of design tile layer #{} in group #{} in {}",
            self.layer_index,
            self.group_index,
            if self.is_background {
                "background"
            } else {
                "foreground"
            }
        )
    }

    fn redo_info(&self) -> String {
        format!(
            "Change attributes of design tile layer #{} in group #{} in {}",
            self.layer_index,
            self.group_index,
            if self.is_background {
                "background"
            } else {
                "foreground"
            }
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActChangeQuadLayerAttr {
    pub is_background: bool,
    pub group_index: usize,
    pub layer_index: usize,
    pub old_attr: MapLayerQuadsAttrs,
    pub new_attr: MapLayerQuadsAttrs,
}

impl EditorActionInterface for ActChangeQuadLayerAttr {
    fn undo_info(&self) -> String {
        format!(
            "Change attributes of quad layer #{} in group #{} in {}",
            self.layer_index,
            self.group_index,
            if self.is_background {
                "background"
            } else {
                "foreground"
            }
        )
    }

    fn redo_info(&self) -> String {
        format!(
            "Change attributes of quad layer #{} in group #{} in {}",
            self.layer_index,
            self.group_index,
            if self.is_background {
                "background"
            } else {
                "foreground"
            }
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActChangeSoundLayerAttr {
    pub is_background: bool,
    pub group_index: usize,
    pub layer_index: usize,
    pub old_attr: MapLayerSoundAttrs,
    pub new_attr: MapLayerSoundAttrs,
}

impl EditorActionInterface for ActChangeSoundLayerAttr {
    fn undo_info(&self) -> String {
        format!(
            "Change attributes of sound layer #{} in group #{} in {}",
            self.layer_index,
            self.group_index,
            if self.is_background {
                "background"
            } else {
                "foreground"
            }
        )
    }

    fn redo_info(&self) -> String {
        format!(
            "Change attributes of sound layer #{} in group #{} in {}",
            self.layer_index,
            self.group_index,
            if self.is_background {
                "background"
            } else {
                "foreground"
            }
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActChangeQuadAttr {
    pub is_background: bool,
    pub group_index: usize,
    pub layer_index: usize,
    pub index: usize,
    pub old_attr: Quad,
    pub new_attr: Quad,
}

impl EditorActionInterface for ActChangeQuadAttr {
    fn undo_info(&self) -> String {
        format!(
            "Change quad attributes #{} in layer #{} in group #{} in {}",
            self.index,
            self.layer_index,
            self.group_index,
            if self.is_background {
                "background"
            } else {
                "foreground"
            }
        )
    }

    fn redo_info(&self) -> String {
        format!(
            "Change quad attributes #{} in layer #{} in group #{} in {}",
            self.index,
            self.layer_index,
            self.group_index,
            if self.is_background {
                "background"
            } else {
                "foreground"
            }
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActChangeSoundAttr {
    pub is_background: bool,
    pub group_index: usize,
    pub layer_index: usize,
    pub index: usize,
    pub old_attr: Sound,
    pub new_attr: Sound,
}

impl EditorActionInterface for ActChangeSoundAttr {
    fn undo_info(&self) -> String {
        format!(
            "Change sound attributes #{} in layer #{} in group #{} in {}",
            self.index,
            self.layer_index,
            self.group_index,
            if self.is_background {
                "background"
            } else {
                "foreground"
            }
        )
    }

    fn redo_info(&self) -> String {
        format!(
            "Change sound attributes #{} in layer #{} in group #{} in {}",
            self.index,
            self.layer_index,
            self.group_index,
            if self.is_background {
                "background"
            } else {
                "foreground"
            }
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActChangeTeleporter {
    pub index: u8,
    pub old_name: String,
    pub new_name: String,
}

impl EditorActionInterface for ActChangeTeleporter {
    fn undo_info(&self) -> String {
        format!(
            "Rename teleporter #{} in tele layer from {} to {}",
            self.index, self.new_name, self.old_name
        )
    }

    fn redo_info(&self) -> String {
        format!(
            "Rename teleporter #{} in tele layer from {} to {}",
            self.index, self.old_name, self.new_name,
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActChangeSwitch {
    pub index: u8,
    pub old_name: String,
    pub new_name: String,
}

impl EditorActionInterface for ActChangeSwitch {
    fn undo_info(&self) -> String {
        format!(
            "Rename switch #{} in switch layer from {} to {}",
            self.index, self.new_name, self.old_name
        )
    }

    fn redo_info(&self) -> String {
        format!(
            "Rename switch #{} in switch layer from {} to {}",
            self.index, self.old_name, self.new_name,
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActChangeTuneZone {
    pub index: u8,
    pub old_name: String,
    pub new_name: String,
    pub old_tunes: LinkedHashMap<String, String>,
    pub new_tunes: LinkedHashMap<String, String>,
}

impl EditorActionInterface for ActChangeTuneZone {
    fn undo_info(&self) -> String {
        format!(
            "Change back tune #{} - {} in tune layer",
            self.index, self.new_name,
        )
    }

    fn redo_info(&self) -> String {
        format!(
            "Change tune #{} - {} in tune layer",
            self.index, self.old_name,
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActAddRemPosAnim {
    pub index: usize,
    pub anim: PosAnimation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActAddPosAnim {
    pub base: ActAddRemPosAnim,
}

impl EditorActionInterface for ActAddPosAnim {
    fn undo_info(&self) -> String {
        if !self.base.anim.name.is_empty() {
            format!(
                "Remove pos animation \"{}\" @{}",
                self.base.anim.name, self.base.index
            )
        } else {
            format!("Remove pos animation @{}", self.base.index)
        }
    }

    fn redo_info(&self) -> String {
        if !self.base.anim.name.is_empty() {
            format!(
                "Add pos animation \"{}\" @{}",
                self.base.anim.name, self.base.index
            )
        } else {
            format!("Add pos animation @{}", self.base.index)
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActRemPosAnim {
    pub base: ActAddRemPosAnim,
}

impl EditorActionInterface for ActRemPosAnim {
    fn undo_info(&self) -> String {
        if !self.base.anim.name.is_empty() {
            format!(
                "Add pos animation \"{}\" @{}",
                self.base.anim.name, self.base.index
            )
        } else {
            format!("Add pos animation @{}", self.base.index)
        }
    }

    fn redo_info(&self) -> String {
        if !self.base.anim.name.is_empty() {
            format!(
                "Remove pos animation \"{}\" @{}",
                self.base.anim.name, self.base.index
            )
        } else {
            format!("Remove pos animation @{}", self.base.index)
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActAddRemColorAnim {
    pub index: usize,
    pub anim: ColorAnimation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActAddColorAnim {
    pub base: ActAddRemColorAnim,
}

impl EditorActionInterface for ActAddColorAnim {
    fn undo_info(&self) -> String {
        if !self.base.anim.name.is_empty() {
            format!(
                "Remove color animation \"{}\" @{}",
                self.base.anim.name, self.base.index
            )
        } else {
            format!("Remove color animation @{}", self.base.index)
        }
    }

    fn redo_info(&self) -> String {
        if !self.base.anim.name.is_empty() {
            format!(
                "Add color animation \"{}\" @{}",
                self.base.anim.name, self.base.index
            )
        } else {
            format!("Add color animation @{}", self.base.index)
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActRemColorAnim {
    pub base: ActAddRemColorAnim,
}

impl EditorActionInterface for ActRemColorAnim {
    fn undo_info(&self) -> String {
        if !self.base.anim.name.is_empty() {
            format!(
                "Add color animation \"{}\" @{}",
                self.base.anim.name, self.base.index
            )
        } else {
            format!("Add color animation @{}", self.base.index)
        }
    }

    fn redo_info(&self) -> String {
        if !self.base.anim.name.is_empty() {
            format!(
                "Remove color animation \"{}\" @{}",
                self.base.anim.name, self.base.index
            )
        } else {
            format!("Remove color animation @{}", self.base.index)
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActAddRemSoundAnim {
    pub index: usize,
    pub anim: SoundAnimation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActAddSoundAnim {
    pub base: ActAddRemSoundAnim,
}

impl EditorActionInterface for ActAddSoundAnim {
    fn undo_info(&self) -> String {
        if !self.base.anim.name.is_empty() {
            format!(
                "Remove sound animation \"{}\" @{}",
                self.base.anim.name, self.base.index
            )
        } else {
            format!("Remove sound animation @{}", self.base.index)
        }
    }

    fn redo_info(&self) -> String {
        if !self.base.anim.name.is_empty() {
            format!(
                "Add sound animation \"{}\" @{}",
                self.base.anim.name, self.base.index
            )
        } else {
            format!("Add sound animation @{}", self.base.index)
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActRemSoundAnim {
    pub base: ActAddRemSoundAnim,
}

impl EditorActionInterface for ActRemSoundAnim {
    fn undo_info(&self) -> String {
        if !self.base.anim.name.is_empty() {
            format!(
                "Add sound animation \"{}\" @{}",
                self.base.anim.name, self.base.index
            )
        } else {
            format!("Add sound animation @{}", self.base.index)
        }
    }

    fn redo_info(&self) -> String {
        if !self.base.anim.name.is_empty() {
            format!(
                "Remove sound animation \"{}\" @{}",
                self.base.anim.name, self.base.index
            )
        } else {
            format!("Remove sound animation @{}", self.base.index)
        }
    }
}
