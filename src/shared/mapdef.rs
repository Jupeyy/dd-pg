use std::mem::size_of;

use num_derive::FromPrimitive;

use math::math::vector::{ivec4, vec2_base};
pub trait ReadFromSlice {
    fn read_from_slice(data: &[u8]) -> Self;
}

// layer types
#[derive(PartialEq, PartialOrd)]
pub enum MapLayerTypes {
    LAYERTYPE_INVALID = 0,
    LAYERTYPE_GAME,
    LAYERTYPE_TILES,
    LAYERTYPE_QUADS,
    LAYERTYPE_FRONT,
    LAYERTYPE_TELE,
    LAYERTYPE_SPEEDUP,
    LAYERTYPE_SWITCH,
    LAYERTYPE_TUNE,
    LAYERTYPE_SOUNDS_DEPRECATED, // deprecated! do not use this, this is just for compatibility reasons
    LAYERTYPE_SOUNDS,
}

#[derive(FromPrimitive)]
pub enum MapItemTypes {
    // TODO(Shereef Marzouk): fix this for vanilla, make use of LAYERTYPE_GAME instead of using m_game variable in the editor.
    MAPITEMTYPE_VERSION = 0,
    MAPITEMTYPE_INFO,
    MAPITEMTYPE_IMAGE,
    MAPITEMTYPE_ENVELOPE,
    MAPITEMTYPE_GROUP,
    MAPITEMTYPE_LAYER,
    MAPITEMTYPE_ENVPOINTS,
    MAPITEMTYPE_SOUND,
    // High map item type numbers suggest that they use the alternate
    // format with UUIDs. See src/engine/shared/datafile.cpp for some of
    // the implementation.
    MAPITEMTYPE_COUNT,
}

pub enum CurveType {
    CURVETYPE_STEP = 0,
    CURVETYPE_LINEAR,
    CURVETYPE_SLOW,
    CURVETYPE_FAST,
    CURVETYPE_SMOOTH,
    NUM_CURVETYPES,
}

pub enum EEntityTiles {
    // game layer tiles
    // TODO define which Layer uses which tiles (needed for mapeditor)
    ENTITY_SPAWN = 192,
    ENTITY_SPAWN_RED,
    ENTITY_SPAWN_BLUE,
}
/*
ENTITY_FLAGSTAND_RED,
ENTITY_FLAGSTAND_BLUE,
ENTITY_ARMOR_1,
ENTITY_HEALTH_1,
ENTITY_WEAPON_SHOTGUN,
ENTITY_WEAPON_GRENADE,
ENTITY_POWERUP_NINJA,
ENTITY_WEAPON_LASER,
//DDRace - Main Lasers
ENTITY_LASER_FAST_CCW,
ENTITY_LASER_NORMAL_CCW,
ENTITY_LASER_SLOW_CCW,
ENTITY_LASER_STOP,
ENTITY_LASER_SLOW_CW,
ENTITY_LASER_NORMAL_CW,
ENTITY_LASER_FAST_CW,
//DDRace - Laser Modifiers
ENTITY_LASER_SHORT,
ENTITY_LASER_MEDIUM,
ENTITY_LASER_LONG,
ENTITY_LASER_C_SLOW,
ENTITY_LASER_C_NORMAL,
ENTITY_LASER_C_FAST,
ENTITY_LASER_O_SLOW,
ENTITY_LASER_O_NORMAL,
ENTITY_LASER_O_FAST,
//DDRace - Plasma
ENTITY_PLASMAE = 29,
ENTITY_PLASMAF,
ENTITY_PLASMA,
ENTITY_PLASMAU,
//DDRace - Shotgun
ENTITY_CRAZY_SHOTGUN_EX,
ENTITY_CRAZY_SHOTGUN,
//DDNet - Removing specific weapon
ENTITY_ARMOR_SHOTGUN,
ENTITY_ARMOR_GRENADE,
ENTITY_ARMOR_NINJA,
ENTITY_ARMOR_LASER,
//DDRace - Draggers
ENTITY_DRAGGER_WEAK = 42,
ENTITY_DRAGGER_NORMAL,
ENTITY_DRAGGER_STRONG,
//Draggers Behind Walls
ENTITY_DRAGGER_WEAK_NW,
ENTITY_DRAGGER_NORMAL_NW,
ENTITY_DRAGGER_STRONG_NW,
//Doors
ENTITY_DOOR = 49,
//End Of Lower Tiles
NUM_ENTITIES,
//Start From Top Left
//Tile Controllers*/

pub fn read_u32_le(data: &[u8]) -> u32 {
    u32::from_le_bytes([data[0], data[1], data[2], data[3]])
}

pub fn read_i32_le(data: &[u8]) -> i32 {
    i32::from_le_bytes([data[0], data[1], data[2], data[3]])
}

pub fn read_i16_le(data: &[u8]) -> i16 {
    i16::from_le_bytes([data[0], data[1]])
}

#[repr(C)]
pub struct CMapItemVersion {
    version: i32,
}

impl CMapItemVersion {
    pub fn read_from_slice(data: &[u8]) -> Self {
        let v = read_i32_le(data);

        Self { version: v }
    }
}

#[repr(C)]
pub struct CMapItemInfo {
    version: i32,
    author: i32,
    map_version: i32,
    credits: i32,
    license: i32,
}

impl CMapItemInfo {
    pub fn read_from_slice(data: &[u8]) -> Self {
        let (ver, rest) = data.split_at(size_of::<i32>());
        let v = read_i32_le(ver);

        let (author, rest) = rest.split_at(size_of::<i32>());
        let author = read_i32_le(author);

        let (map_ver, rest) = rest.split_at(size_of::<i32>());
        let map_ver = read_i32_le(map_ver);

        let (credits, rest) = rest.split_at(size_of::<i32>());
        let credits = read_i32_le(credits);

        let (license, _rest) = rest.split_at(size_of::<i32>());
        let license = read_i32_le(license);

        Self {
            version: v,
            author,
            map_version: map_ver,
            credits,
            license,
        }
    }
}

#[repr(C)]
pub struct CMapItemInfoSettings {
    info: CMapItemInfo,
    settings: i32,
}

impl CMapItemInfoSettings {
    pub fn read_from_slice(data: &[u8]) -> Self {
        let (info, rest) = data.split_at(size_of::<CMapItemInfo>());
        let info = CMapItemInfo::read_from_slice(info);

        let (settings, _rest) = rest.split_at(size_of::<i32>());
        let settings = read_i32_le(settings);

        Self {
            info: info,
            settings,
        }
    }
}

#[derive(Copy, Clone, Default)]
#[repr(C)]
pub struct CMapItemImage {
    pub version: i32,
    pub width: i32,
    pub height: i32,
    pub external: i32,
    pub image_name: i32,
    pub image_data: i32,
}

impl CMapItemImage {
    pub fn read_from_slice(data: &[u8]) -> Self {
        let (version, rest) = data.split_at(size_of::<i32>());
        let version = read_i32_le(version);

        let (width, rest) = rest.split_at(size_of::<i32>());
        let width = read_i32_le(width);

        let (height, rest) = rest.split_at(size_of::<i32>());
        let height = read_i32_le(height);

        let (external, rest) = rest.split_at(size_of::<i32>());
        let external = read_i32_le(external);

        let (image_name, rest) = rest.split_at(size_of::<i32>());
        let image_name = read_i32_le(image_name);

        let (image_data, _rest) = rest.split_at(size_of::<i32>());
        let image_data = read_i32_le(image_data);

        Self {
            version,
            width,
            height,
            external,
            image_name,
            image_data,
        }
    }
}

#[derive(Default)]
pub struct MapImage {
    pub item_data: CMapItemImage,
    pub img_data: &'static mut [u8],
    pub img_used: bool,
    pub img_3d_data: &'static mut [u8],
    pub img_3d_used: bool,
    pub img_3d_width: usize,
    pub img_3d_height: usize,
    pub img_3d_depth: usize,
    pub img_name: String,
}

pub const TILE_SWITCHTIMEDOPEN: u8 = 22;
pub enum TileNum {
    TILE_AIR = 0,
    TILE_SOLID,
    TILE_DEATH,
    TILE_NOHOOK,
    TILE_NOLASER,
    TILE_THROUGH_CUT,
    TILE_THROUGH,
    TILE_JUMP,
    TILE_FREEZE = 9,
    TILE_TELEINEVIL,
    TILE_UNFREEZE,
    TILE_DFREEZE,
    TILE_DUNFREEZE,
    TILE_TELEINWEAPON,
    TILE_TELEINHOOK,
    TILE_WALLJUMP = 16,
    TILE_EHOOK_ENABLE,
    TILE_EHOOK_DISABLE,
    TILE_HIT_ENABLE,
    TILE_HIT_DISABLE,
    TILE_SOLO_ENABLE,
    TILE_SOLO_DISABLE,
    //Switches
    TILE_SWITCHTIMEDCLOSE,
    TILE_SWITCHOPEN,
    TILE_SWITCHCLOSE,
    TILE_TELEIN,
    TILE_TELEOUT,
    TILE_BOOST,
    TILE_TELECHECK,
    TILE_TELECHECKOUT,
    TILE_TELECHECKIN,
    TILE_REFILL_JUMPS = 32,
    TILE_START,
    TILE_FINISH,
    TILE_TIME_CHECKPOINT_FIRST = 35,
    TILE_TIME_CHECKPOINT_LAST = 59,
    TILE_STOP = 60,
    TILE_STOPS,
    TILE_STOPA,
    TILE_TELECHECKINEVIL,
    TILE_CP,
    TILE_CP_F,
    TILE_THROUGH_ALL,
    TILE_THROUGH_DIR,
    TILE_TUNE,
    TILE_OLDLASER = 71,
    TILE_NPC,
    TILE_EHOOK,
    TILE_NOHIT,
    TILE_NPH,
    TILE_UNLOCK_TEAM,
    TILE_ADD_TIME = 79,
    TILE_NPC_DISABLE = 88,
    TILE_UNLIMITED_JUMPS_DISABLE,
    TILE_JETPACK_DISABLE,
    TILE_NPH_DISABLE,
    TILE_SUBTRACT_TIME = 95,
    TILE_TELE_GUN_ENABLE = 96,
    TILE_TELE_GUN_DISABLE = 97,
    TILE_ALLOW_TELE_GUN = 98,
    TILE_ALLOW_BLUE_TELE_GUN = 99,
    TILE_NPC_ENABLE = 104,
    TILE_UNLIMITED_JUMPS_ENABLE,
    TILE_JETPACK_ENABLE,
    TILE_NPH_ENABLE,
    TILE_TELE_GRENADE_ENABLE = 112,
    TILE_TELE_GRENADE_DISABLE = 113,
    TILE_TELE_LASER_ENABLE = 128,
    TILE_TELE_LASER_DISABLE = 129,
    TILE_CREDITS_1 = 140,
    TILE_CREDITS_2 = 141,
    TILE_CREDITS_3 = 142,
    TILE_CREDITS_4 = 143,
    TILE_LFREEZE = 144,
    TILE_LUNFREEZE = 145,
    TILE_CREDITS_5 = 156,
    TILE_CREDITS_6 = 157,
    TILE_CREDITS_7 = 158,
    TILE_CREDITS_8 = 159,
    TILE_ENTITIES_OFF_1 = 190,
    TILE_ENTITIES_OFF_2,
}
/*
//End of higher tiles
//Layers
LAYER_GAME = 0,
LAYER_FRONT,
LAYER_TELE,
LAYER_SPEEDUP,
LAYER_SWITCH,
LAYER_TUNE,
NUM_LAYERS,
*/

pub enum TileFlag {
    //Flags
    XFLIP = 1,
    YFLIP = 2,
    OPAQUE = 4,
    ROTATE = 8,
}
/*
//Rotation
ROTATION_0 = 0,
ROTATION_90 = TILEFLAG_ROTATE,
ROTATION_180 = (TILEFLAG_XFLIP | TILEFLAG_YFLIP),
ROTATION_270 = (TILEFLAG_XFLIP | TILEFLAG_YFLIP | TILEFLAG_ROTATE),

ENTITY_OFFSET = 255 - 16 * 4,*/

pub enum LayerFlag {
    LAYERFLAG_DETAIL = 1,
}

pub enum TilesLayerFlag {
    TILESLAYERFLAG_GAME = 1,
    TILESLAYERFLAG_TELE = 2,
    TILESLAYERFLAG_SPEEDUP = 4,
    TILESLAYERFLAG_FRONT = 8,
    TILESLAYERFLAG_SWITCH = 16,
    TILESLAYERFLAG_TUNE = 32,
}

pub enum ItemType {
    ITEMTYPE_EX = 0xffff,
}

pub enum CMapItemGroupExVer {
    CURRENT_VERSION = 1,
}

#[derive(Default)]
#[repr(C)]
pub struct CMapItemGroupEx {
    version: i32,

    // ItemGroup's perceived distance from camera when zooming. Similar to how
    // Parallax{X,Y} works when camera is moving along the X and Y axes,
    // this setting applies to camera moving closer or away (zooming in or out).
    pub parallax_zoom: i32,
}

pub enum CMapItemGroupVer {
    CURRENT_VERSION = 3,
}

#[derive(Default, Clone)]
#[repr(C)]
pub struct CMapItemGroup {
    pub version: i32,
    pub offset_x: i32,
    pub offset_y: i32,
    pub parallax_x: i32,
    pub parallax_y: i32,

    pub start_layer: i32,
    pub num_layers: i32,

    pub use_clipping: i32,
    pub clip_x: i32,
    pub clip_y: i32,
    pub clip_w: i32,
    pub clip_h: i32,

    name: [i32; 3],
}

impl CMapItemGroup {
    pub fn read_from_slice(data: &[u8]) -> Self {
        let (version, rest) = data.split_at(size_of::<i32>());
        let version = read_i32_le(version);

        let (offset_x, rest) = rest.split_at(size_of::<i32>());
        let offset_x = read_i32_le(offset_x);

        let (offset_y, rest) = rest.split_at(size_of::<i32>());
        let offset_y = read_i32_le(offset_y);

        let (parallax_x, rest) = rest.split_at(size_of::<i32>());
        let parallax_x = read_i32_le(parallax_x);

        let (parallax_y, rest) = rest.split_at(size_of::<i32>());
        let parallax_y = read_i32_le(parallax_y);

        let (start_layer, rest) = rest.split_at(size_of::<i32>());
        let start_layer = read_i32_le(start_layer);

        let (num_layers, rest) = rest.split_at(size_of::<i32>());
        let num_layers = read_i32_le(num_layers);

        let (use_clipping, rest) = rest.split_at(size_of::<i32>());
        let use_clipping = read_i32_le(use_clipping);

        let (clip_x, rest) = rest.split_at(size_of::<i32>());
        let clip_x = read_i32_le(clip_x);

        let (clip_y, rest) = rest.split_at(size_of::<i32>());
        let clip_y = read_i32_le(clip_y);

        let (clip_w, rest) = rest.split_at(size_of::<i32>());
        let clip_w = read_i32_le(clip_w);

        let (clip_h, mut rest) = rest.split_at(size_of::<i32>());
        let clip_h = read_i32_le(clip_h);

        let mut iname: [i32; 3] = Default::default();
        iname.iter_mut().for_each(|c| {
            let (name, rest2) = rest.split_at(size_of::<i32>());
            *c = read_i32_le(name);
            rest = rest2;
        });

        Self {
            version,
            offset_x,
            offset_y,
            parallax_x,
            parallax_y,

            start_layer,
            num_layers,

            use_clipping,
            clip_x,
            clip_y,
            clip_w,
            clip_h,

            name: iname,
        }
    }
}

#[derive(Default, Clone)]
#[repr(C)]
pub struct CMapItemLayer {
    version: i32,
    pub item_layer: i32,
    pub flags: i32,
}

impl CMapItemLayer {
    pub fn read_from_slice(data: &[u8]) -> Self {
        let (version, rest) = data.split_at(size_of::<i32>());
        let version = read_i32_le(version);

        let (itype, rest) = rest.split_at(size_of::<i32>());
        let itype = read_i32_le(itype);

        let (flags, _rest) = rest.split_at(size_of::<i32>());
        let flags = read_i32_le(flags);

        Self {
            version,
            item_layer: itype,
            flags,
        }
    }
}

type CColor = ivec4;

#[derive(Default, Clone)]
#[repr(C)]
pub struct CMapItemLayerTilemap {
    layer: CMapItemLayer,
    pub version: i32,

    pub width: i32,
    pub height: i32,
    pub flags: i32,

    pub color: CColor,
    pub color_env: i32,
    pub color_env_offset: i32,

    pub image: i32,
    pub data: i32,

    name: [i32; 3],

    // DDRace
    pub tele: i32,
    pub speedup: i32,
    pub front: i32,
    pub switch: i32,
    pub tune: i32,
}

impl CMapItemLayerTilemap {
    pub fn read_from_slice(data: &[u8]) -> Self {
        let (tile_layer, rest) = data.split_at(size_of::<CMapItemLayer>());
        let tile_layer = CMapItemLayer::read_from_slice(tile_layer);

        let (version, rest) = rest.split_at(size_of::<i32>());
        let version = read_i32_le(version);

        let (width, rest) = rest.split_at(size_of::<i32>());
        let width = read_i32_le(width);

        let (height, rest) = rest.split_at(size_of::<i32>());
        let height = read_i32_le(height);

        let (flags, rest) = rest.split_at(size_of::<i32>());
        let flags = read_i32_le(flags);

        let (color, rest) = rest.split_at(size_of::<CColor>());
        let color = CColor::read_from_slice(color);

        let (color_env, rest) = rest.split_at(size_of::<i32>());
        let color_env = read_i32_le(color_env);

        let (color_env_off, rest) = rest.split_at(size_of::<i32>());
        let color_env_off = read_i32_le(color_env_off);

        let (img, rest) = rest.split_at(size_of::<i32>());
        let img = read_i32_le(img);

        let (data, mut rest) = rest.split_at(size_of::<i32>());
        let data = read_i32_le(data);

        let mut name: [i32; 3] = Default::default();
        name.iter_mut().for_each(|n| {
            let (c, rest2) = rest.split_at(size_of::<i32>());
            *n = read_i32_le(c);
            rest = rest2;
        });

        let mut tele = -1;
        if rest.len() > 0 {
            let (tel, rest2) = rest.split_at(size_of::<i32>());
            tele = read_i32_le(tel);
            rest = rest2;
        }

        let mut speedup = -1;
        if rest.len() > 0 {
            let (val, rest2) = rest.split_at(size_of::<i32>());
            speedup = read_i32_le(val);
            rest = rest2;
        }

        let mut front = -1;
        if rest.len() > 0 {
            let (val, rest2) = rest.split_at(size_of::<i32>());
            front = read_i32_le(val);
            rest = rest2;
        }

        let mut switch = -1;
        if rest.len() > 0 {
            let (val, rest2) = rest.split_at(size_of::<i32>());
            switch = read_i32_le(val);
            rest = rest2;
        }

        let mut tune = -1;
        if rest.len() > 0 {
            let (val, rest2) = rest.split_at(size_of::<i32>());
            tune = read_i32_le(val);
            rest = rest2;
        }

        Self {
            layer: tile_layer,
            version,

            width,
            height,
            flags,

            color,
            color_env,
            color_env_offset: color_env_off,

            image: img,
            data,

            name,

            // DDRace
            tele,
            speedup,
            front,
            switch,
            tune,
        }
    }
}

#[derive(Default, Clone)]
#[repr(C)]
pub struct CMapItemLayerQuads {
    pub layer: CMapItemLayer,
    pub version: i32,

    pub num_quads: i32,
    pub data: i32,
    pub image: i32,

    pub name: [i32; 3],
}

impl CMapItemLayerQuads {
    pub fn read_from_slice(data: &[u8]) -> Self {
        let (tile_layer, rest) = data.split_at(size_of::<CMapItemLayer>());
        let tile_layer = CMapItemLayer::read_from_slice(tile_layer);

        let (version, rest) = rest.split_at(size_of::<i32>());
        let version = read_i32_le(version);

        let (num_quads, rest) = rest.split_at(size_of::<i32>());
        let num_quads = read_i32_le(num_quads);

        let (data, rest) = rest.split_at(size_of::<i32>());
        let data = read_i32_le(data);

        let (img, mut rest) = rest.split_at(size_of::<i32>());
        let img = read_i32_le(img);

        let mut name: [i32; 3] = Default::default();
        name.iter_mut().for_each(|n| {
            let (c, rest2) = rest.split_at(size_of::<i32>());
            *n = read_i32_le(c);
            rest = rest2;
        });

        Self {
            layer: tile_layer,
            version,

            num_quads,

            data,
            image: img,

            name,
        }
    }
}

enum CMapItemLayerSoundsVer {
    CURRENT_VERSION = 2,
}

#[derive(Default, Clone)]
#[repr(C)]
pub struct CMapItemLayerSounds {
    layer: CMapItemLayer,
    version: i32,

    num_sources: i32,
    data: i32,
    sound: i32,

    name: [i32; 3],
}

impl CMapItemLayerSounds {
    pub fn read_from_slice(data: &[u8]) -> Self {
        let (tile_layer, rest) = data.split_at(size_of::<CMapItemLayer>());
        let tile_layer = CMapItemLayer::read_from_slice(tile_layer);

        let (version, rest) = rest.split_at(size_of::<i32>());
        let version = read_i32_le(version);

        let (num_sources, rest) = rest.split_at(size_of::<i32>());
        let num_sources = read_i32_le(num_sources);

        let (data, rest) = rest.split_at(size_of::<i32>());
        let data = read_i32_le(data);

        let (snd, mut rest) = rest.split_at(size_of::<i32>());
        let snd = read_i32_le(snd);

        let mut name: [i32; 3] = Default::default();
        name.iter_mut().for_each(|n| {
            let (c, rest2) = rest.split_at(size_of::<i32>());
            *n = read_i32_le(c);
            rest = rest2;
        });

        Self {
            layer: tile_layer,
            version,

            num_sources,

            data,
            sound: snd,

            name,
        }
    }
}

#[derive(Copy, Clone, Default)]
#[repr(C)]
pub struct CTile {
    pub index: u8,
    pub flags: u8,
    pub skip: u8,
    pub reserved: u8,
}

impl ReadFromSlice for CTile {
    fn read_from_slice(data: &[u8]) -> Self {
        let (index, rest) = data.split_at(size_of::<u8>());
        let index = index[0];

        let (flags, rest) = rest.split_at(size_of::<u8>());
        let flags = flags[0];

        let (skip, rest) = rest.split_at(size_of::<u8>());
        let skip = skip[0];

        let (reserved, _rest) = rest.split_at(size_of::<u8>());
        let reserved = reserved[0];

        Self {
            index,
            flags,
            skip,
            reserved,
        }
    }
}

// ddrace
#[derive(Default, Clone)]
#[repr(C)]
pub struct CTeleTile {
    pub number: u8,
    pub tile_type: u8,
}

impl ReadFromSlice for CTeleTile {
    fn read_from_slice(data: &[u8]) -> Self {
        let (number, rest) = data.split_at(size_of::<u8>());
        let number = number[0];

        let (ttype, _rest) = rest.split_at(size_of::<u8>());
        let ttype = ttype[0];

        Self {
            number,
            tile_type: ttype,
        }
    }
}

#[derive(Default, Clone)]
#[repr(C)]
pub struct CSpeedupTile {
    pub force: u8,
    pub max_speed: u8,
    pub tile_type: u8,
    pub angle: i16,
}

impl ReadFromSlice for CSpeedupTile {
    fn read_from_slice(data: &[u8]) -> Self {
        let (force, rest) = data.split_at(size_of::<u8>());
        let force = force[0];

        let (max_speed, rest) = rest.split_at(size_of::<u8>());
        let max_speed = max_speed[0];

        let (ttype, rest) = rest.split_at(size_of::<u8>());
        let ttype = ttype[0];

        let (angle, _rest) = rest.split_at(size_of::<i16>());
        let angle = read_i16_le(angle);

        Self {
            force,
            max_speed,
            tile_type: ttype,
            angle,
        }
    }
}

#[derive(Default, Clone)]
#[repr(C)]
pub struct CSwitchTile {
    pub number: u8,
    pub tile_type: u8,
    pub flags: u8,
    pub delay: u8,
}

impl ReadFromSlice for CSwitchTile {
    fn read_from_slice(data: &[u8]) -> Self {
        let (number, rest) = data.split_at(size_of::<u8>());
        let number = number[0];

        let (ttype, rest) = rest.split_at(size_of::<u8>());
        let ttype = ttype[0];

        let (flags, rest) = rest.split_at(size_of::<u8>());
        let flags = flags[0];

        let (delay, _rest) = rest.split_at(size_of::<u8>());
        let delay = delay[0];

        Self {
            number,
            tile_type: ttype,
            flags,
            delay,
        }
    }
}

#[derive(Default, Clone)]
#[repr(C)]
pub struct CDoorTile {
    pub index: u8,
    pub flags: u8,
    pub number: i32,
}

impl ReadFromSlice for CDoorTile {
    fn read_from_slice(data: &[u8]) -> Self {
        let (index, rest) = data.split_at(size_of::<u8>());
        let index = index[0];

        let (flags, rest) = rest.split_at(size_of::<u8>());
        let flags = flags[0];

        let (number, _rest) = rest.split_at(size_of::<i32>());
        let number = read_i32_le(number);

        Self {
            index,
            flags,
            number,
        }
    }
}

#[derive(Default, Clone)]
#[repr(C)]
pub struct CTuneTile {
    pub number: u8,
    pub tile_type: u8,
}

impl ReadFromSlice for CTuneTile {
    fn read_from_slice(data: &[u8]) -> Self {
        let (number, rest) = data.split_at(size_of::<u8>());
        let _number = number[0];

        let (ttype, _rest) = rest.split_at(size_of::<u8>());
        let ttype = ttype[0];

        Self {
            number: ttype,
            tile_type: ttype,
        }
    }
}

pub type CPoint = vec2_base<i32>;

impl ReadFromSlice for CPoint {
    fn read_from_slice(data: &[u8]) -> Self {
        let (x, rest) = data.split_at(size_of::<i32>());
        let x = read_i32_le(x);

        let (y, _rest) = rest.split_at(size_of::<i32>());
        let y = read_i32_le(y);

        Self { x: x, y: y }
    }
}

#[derive(Default, Clone)]
pub struct CQuad {
    pub points: [CPoint; 5],
    pub colors: [CColor; 4],
    pub tex_coords: [CPoint; 4],

    pub pos_env: i32,
    pub pos_env_offset: i32,

    pub color_env: i32,
    pub color_env_offset: i32,
}

impl ReadFromSlice for CQuad {
    fn read_from_slice(data: &[u8]) -> Self {
        let mut rest = data;

        let mut points: [CPoint; 5] = Default::default();
        points.iter_mut().for_each(|point| {
            let (point_slice, rest_inner) = rest.split_at(size_of::<CPoint>());
            *point = CPoint::read_from_slice(point_slice);
            rest = rest_inner;
        });

        let mut colors: [CColor; 4] = Default::default();
        colors.iter_mut().for_each(|color| {
            let (point_slice, rest_inner) = rest.split_at(size_of::<CColor>());
            *color = CColor::read_from_slice(point_slice);
            rest = rest_inner;
        });

        let mut tex_coords: [CPoint; 4] = Default::default();
        tex_coords.iter_mut().for_each(|tex_coord| {
            let (tex_coord_slice, rest_inner) = rest.split_at(size_of::<CPoint>());
            *tex_coord = CPoint::read_from_slice(tex_coord_slice);
            rest = rest_inner;
        });

        let (pos_env, rest) = rest.split_at(size_of::<i32>());
        let pos_env = read_i32_le(pos_env);

        let (pos_env_off, rest) = rest.split_at(size_of::<i32>());
        let pos_env_off = read_i32_le(pos_env_off);

        let (color_env, rest) = rest.split_at(size_of::<i32>());
        let color_env = read_i32_le(color_env);

        let (color_env_off, _rest) = rest.split_at(size_of::<i32>());
        let color_env_off = read_i32_le(color_env_off);

        Self {
            points,
            colors,
            tex_coords,
            pos_env,
            pos_env_offset: pos_env_off,
            color_env,
            color_env_offset: color_env_off,
        }
    }
}

#[derive(Clone)]
pub enum MapTileLayerDetail {
    Tile(),
    Tele(Vec<CTeleTile>),
    Speedup(Vec<CSpeedupTile>),
    Switch(Vec<CSwitchTile>),
    Door(Vec<CDoorTile>),
    Tune(Vec<CTuneTile>),
}

#[derive(Clone)]
pub struct MapLayerTile(
    pub CMapItemLayerTilemap,
    pub MapTileLayerDetail,
    pub Vec<CTile>,
);

#[derive(Clone)]
pub struct MapLayerQuad(pub CMapItemLayerQuads, pub Vec<CQuad>);

#[derive(Clone)]
pub enum MapLayer {
    Tile(MapLayerTile),
    Quads(MapLayerQuad),
    Sound(CMapItemLayerSounds),
    Unknown(CMapItemLayer),
}

impl MapLayer {
    pub fn get_tile_layer_base(&self) -> &CMapItemLayer {
        match self {
            MapLayer::Tile(layer) => &layer.0.layer,
            MapLayer::Quads(layer) => &layer.0.layer,
            MapLayer::Sound(layer) => &layer.layer,
            MapLayer::Unknown(layer) => &layer,
        }
    }
}

#[repr(C)]
pub struct CEnvPoint {
    pub time: i32, // in ms
    pub curve_type: i32,
    pub values: [i32; 4], // 1-4 depending on envelope (22.10 fixed point)
}

impl CEnvPoint {
    pub fn read_from_slice(data: &[u8]) -> Self {
        let (version, rest) = data.split_at(size_of::<i32>());
        let time = read_i32_le(version);

        let (curve_type, mut rest) = rest.split_at(size_of::<i32>());
        let itype = read_i32_le(curve_type);

        let mut values: [i32; 4] = Default::default();
        values.iter_mut().for_each(|c| {
            let (val, rest2) = rest.split_at(size_of::<i32>());
            *c = read_i32_le(val);
            rest = rest2;
        });

        Self {
            time,
            curve_type: itype,
            values,
        }
    }
}

enum CMapItemEnvelopeVer {
    CURRENT_VERSION = 2,
}

#[derive(Default)]
#[repr(C)]
pub struct CMapItemEnvelope {
    pub version: i32,
    pub channels: i32,
    pub start_point: i32,
    pub num_points: i32,
    pub name: [i32; 8],
    pub synchronized: i32,
}

impl CMapItemEnvelope {
    pub fn read_from_slice(data: &[u8]) -> Self {
        let (version, rest) = data.split_at(size_of::<i32>());
        let version = read_i32_le(version);

        let (channels, rest) = rest.split_at(size_of::<i32>());
        let channels = read_i32_le(channels);

        let (start_point, rest) = rest.split_at(size_of::<i32>());
        let start_point = read_i32_le(start_point);

        let (num_points, mut rest) = rest.split_at(size_of::<i32>());
        let num_points = read_i32_le(num_points);

        let mut iname: [i32; 8] = Default::default();
        iname.iter_mut().for_each(|c| {
            let (name, rest2) = rest.split_at(size_of::<i32>());
            *c = read_i32_le(name);
            rest = rest2;
        });

        let (synchronized, _rest) = rest.split_at(size_of::<i32>());
        let synchronized = read_i32_le(synchronized);

        Self {
            version: version,
            channels: channels,
            start_point: start_point,
            num_points: num_points,
            name: iname,
            synchronized: synchronized,
        }
    }
}

#[repr(C)]
pub struct CMapItemSound {
    version: i32,

    external: i32,

    sound_name: i32,
    sound_data: i32,
    sound_data_size: i32,
}

impl CMapItemSound {
    pub fn read_from_slice(data: &[u8]) -> Self {
        let (version, rest) = data.split_at(size_of::<i32>());
        let version = read_i32_le(version);

        let (external, rest) = rest.split_at(size_of::<i32>());
        let external = read_i32_le(external);

        let (sound_name, rest) = rest.split_at(size_of::<i32>());
        let sound_name = read_i32_le(sound_name);

        let (sound_data, rest) = rest.split_at(size_of::<i32>());
        let sound_data = read_i32_le(sound_data);

        let (sound_data_size, _rest) = rest.split_at(size_of::<i32>());
        let sound_data_size = read_i32_le(sound_data_size);

        Self {
            version,

            external,

            sound_name,
            sound_data,
            sound_data_size,
        }
    }
}
