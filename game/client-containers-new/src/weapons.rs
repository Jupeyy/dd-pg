use std::sync::Arc;

use game_interface::types::weapons::WeaponType;
use graphics::{
    graphics_mt::GraphicsMultiThreaded,
    handles::texture::texture::{GraphicsTextureHandle, TextureContainer},
};
use graphics_types::{
    commands::{TexFlags, TexFormat},
    types::ImageFormat,
};
use sound::{
    sound_handle::SoundObjectHandle, sound_mt::SoundMultiThreaded,
    sound_mt_types::SoundBackendMemory, sound_object::SoundObject,
};

use crate::container::{
    load_sound_file_part_and_upload, ContainerLoadedItem, ContainerLoadedItemDir,
};

use super::container::{
    load_file_part_and_upload, Container, ContainerItemLoadData, ContainerLoad,
};

#[derive(Debug, Clone)]
pub struct Weapon {
    pub tex: TextureContainer,
    pub cursor: TextureContainer,
    pub projectiles: Vec<TextureContainer>,
    pub muzzles: Vec<TextureContainer>,

    pub fire: [SoundObject; 3],
    pub switch: [SoundObject; 3],
    pub noammo: [SoundObject; 5],
}

#[derive(Debug, Clone)]
pub struct Grenade {
    pub weapon: Weapon,
    pub spawn: SoundObject,
    pub collect: SoundObject,
    pub explosions: [SoundObject; 3],
}

#[derive(Debug, Clone)]
pub struct Laser {
    pub weapon: Weapon,
    pub spawn: SoundObject,
    pub collect: SoundObject,
    pub bounces: [SoundObject; 3],
    pub heads: [TextureContainer; 3],
}

#[derive(Debug, Clone)]
pub struct Shotgun {
    pub weapon: Weapon,
    pub spawn: SoundObject,
    pub collect: SoundObject,
}

#[derive(Debug, Clone)]
pub struct Hammer {
    pub weapon: Weapon,
    pub hits: [SoundObject; 3],
}

#[derive(Debug, Clone)]
pub struct Weapons {
    pub hammer: Hammer,
    pub gun: Weapon,
    pub shotgun: Shotgun,
    pub grenade: Grenade,
    pub laser: Laser,
}

impl Weapons {
    pub fn by_type(&self, weapon: WeaponType) -> &Weapon {
        match weapon {
            WeaponType::Hammer => &self.hammer.weapon,
            WeaponType::Gun => &self.gun,
            WeaponType::Shotgun => &self.shotgun.weapon,
            WeaponType::Grenade => &self.grenade.weapon,
            WeaponType::Laser => &self.laser.weapon,
        }
    }
}

#[derive(Debug)]
pub struct LoadWeapon {
    tex: ContainerItemLoadData,
    cursor_tex: ContainerItemLoadData,
    projectiles: Vec<ContainerItemLoadData>,
    muzzles: Vec<ContainerItemLoadData>,

    fire: [SoundBackendMemory; 3],
    switch: [SoundBackendMemory; 3],
    noammo: [SoundBackendMemory; 5],
}

impl LoadWeapon {
    pub fn load_weapon(
        graphics_mt: &GraphicsMultiThreaded,
        sound_mt: &SoundMultiThreaded,
        files: &ContainerLoadedItemDir,
        default_files: &ContainerLoadedItemDir,
        weapon_name: &str,
        weapon_part: &str,
        projectile_count: usize,
        muzzle_count: usize,
    ) -> anyhow::Result<Self> {
        let mut projectiles: Vec<ContainerItemLoadData> = Default::default();
        let mut muzzles: Vec<ContainerItemLoadData> = Default::default();
        // projectiles
        for proj in 0..projectile_count {
            projectiles.push(load_file_part_and_upload(
                graphics_mt,
                files,
                default_files,
                weapon_name,
                &[weapon_part],
                &("projectile".to_string() + &proj.to_string()),
            )?);
        }
        // muzzles
        for muzzle in 0..muzzle_count {
            muzzles.push(load_file_part_and_upload(
                graphics_mt,
                files,
                default_files,
                weapon_name,
                &[weapon_part],
                &("muzzle".to_string() + &muzzle.to_string()),
            )?);
        }

        Ok(Self {
            // weapon
            tex: load_file_part_and_upload(
                graphics_mt,
                files,
                default_files,
                weapon_name,
                &[weapon_part],
                "weapon",
            )?,
            // cursor
            cursor_tex: load_file_part_and_upload(
                graphics_mt,
                files,
                default_files,
                weapon_name,
                &[weapon_part],
                "cursor",
            )?,

            projectiles,
            muzzles,

            fire: [
                load_sound_file_part_and_upload(
                    sound_mt,
                    files,
                    default_files,
                    weapon_name,
                    &[weapon_part],
                    "fire1",
                )?,
                load_sound_file_part_and_upload(
                    sound_mt,
                    files,
                    default_files,
                    weapon_name,
                    &[weapon_part],
                    "fire2",
                )?,
                load_sound_file_part_and_upload(
                    sound_mt,
                    files,
                    default_files,
                    weapon_name,
                    &[weapon_part],
                    "fire3",
                )?,
            ],
            switch: [
                load_sound_file_part_and_upload(
                    sound_mt,
                    files,
                    default_files,
                    weapon_name,
                    &[weapon_part],
                    "switch1",
                )?,
                load_sound_file_part_and_upload(
                    sound_mt,
                    files,
                    default_files,
                    weapon_name,
                    &[weapon_part],
                    "switch2",
                )?,
                load_sound_file_part_and_upload(
                    sound_mt,
                    files,
                    default_files,
                    weapon_name,
                    &[weapon_part],
                    "switch3",
                )?,
            ],
            noammo: [
                load_sound_file_part_and_upload(
                    sound_mt,
                    files,
                    default_files,
                    weapon_name,
                    &[weapon_part],
                    "noammo1",
                )?,
                load_sound_file_part_and_upload(
                    sound_mt,
                    files,
                    default_files,
                    weapon_name,
                    &[weapon_part],
                    "noammo2",
                )?,
                load_sound_file_part_and_upload(
                    sound_mt,
                    files,
                    default_files,
                    weapon_name,
                    &[weapon_part],
                    "noammo3",
                )?,
                load_sound_file_part_and_upload(
                    sound_mt,
                    files,
                    default_files,
                    weapon_name,
                    &[weapon_part],
                    "noammo4",
                )?,
                load_sound_file_part_and_upload(
                    sound_mt,
                    files,
                    default_files,
                    weapon_name,
                    &[weapon_part],
                    "noammo5",
                )?,
            ],
        })
    }

    fn load_file_into_texture(
        texture_handle: &GraphicsTextureHandle,
        img: ContainerItemLoadData,
        name: &str,
    ) -> TextureContainer {
        texture_handle
            .load_texture(
                img.width as usize,
                img.height as usize,
                ImageFormat::Rgba,
                img.data,
                TexFormat::RGBA,
                TexFlags::empty(),
                name,
            )
            .unwrap()
    }

    fn load_files_into_objects(
        texture_handle: &GraphicsTextureHandle,
        sound_object_handle: &SoundObjectHandle,
        file: LoadWeapon,
        name: &str,
    ) -> Weapon {
        Weapon {
            tex: Self::load_file_into_texture(texture_handle, file.tex, name),
            cursor: Self::load_file_into_texture(texture_handle, file.cursor_tex, name),
            projectiles: file
                .projectiles
                .into_iter()
                .map(|file| Self::load_file_into_texture(texture_handle, file, name))
                .collect(),
            muzzles: file
                .muzzles
                .into_iter()
                .map(|file| Self::load_file_into_texture(texture_handle, file, name))
                .collect(),

            fire: {
                let [fire1, fire2, fire3] = file.fire;
                [
                    sound_object_handle.create(fire1),
                    sound_object_handle.create(fire2),
                    sound_object_handle.create(fire3),
                ]
            },
            switch: {
                let [switch1, switch2, switch3] = file.switch;
                [
                    sound_object_handle.create(switch1),
                    sound_object_handle.create(switch2),
                    sound_object_handle.create(switch3),
                ]
            },
            noammo: {
                let [noammo1, noammo2, noammo3, noammo4, noammo5] = file.noammo;
                [
                    sound_object_handle.create(noammo1),
                    sound_object_handle.create(noammo2),
                    sound_object_handle.create(noammo3),
                    sound_object_handle.create(noammo4),
                    sound_object_handle.create(noammo5),
                ]
            },
        }
    }
}

#[derive(Debug)]
pub struct LoadGrenade {
    weapon: LoadWeapon,

    spawn: SoundBackendMemory,
    collect: SoundBackendMemory,
    explosions: [SoundBackendMemory; 3],
}

impl LoadGrenade {
    pub fn load_weapon(
        graphics_mt: &GraphicsMultiThreaded,
        sound_mt: &SoundMultiThreaded,
        files: &ContainerLoadedItemDir,
        default_files: &ContainerLoadedItemDir,
        weapon_name: &str,
        weapon_part: &str,
        projectile_count: usize,
        muzzle_count: usize,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            weapon: LoadWeapon::load_weapon(
                graphics_mt,
                sound_mt,
                files,
                default_files,
                weapon_name,
                weapon_part,
                projectile_count,
                muzzle_count,
            )?,

            explosions: {
                let mut sounds = Vec::new();
                for i in 0..3 {
                    sounds.push(load_sound_file_part_and_upload(
                        sound_mt,
                        files,
                        default_files,
                        weapon_name,
                        &[weapon_part],
                        &format!("explosion{}", i + 1),
                    )?);
                }
                sounds.try_into().unwrap()
            },

            spawn: load_sound_file_part_and_upload(
                sound_mt,
                files,
                default_files,
                weapon_name,
                &[weapon_part],
                "spawn",
            )?,

            collect: load_sound_file_part_and_upload(
                sound_mt,
                files,
                default_files,
                weapon_name,
                &[weapon_part],
                "collect",
            )?,
        })
    }
}

#[derive(Debug)]
pub struct LoadLaser {
    weapon: LoadWeapon,

    spawn: SoundBackendMemory,
    collect: SoundBackendMemory,
    bounces: [SoundBackendMemory; 3],
    heads: [ContainerItemLoadData; 3],
}

impl LoadLaser {
    pub fn load_weapon(
        graphics_mt: &GraphicsMultiThreaded,
        sound_mt: &SoundMultiThreaded,
        files: &ContainerLoadedItemDir,
        default_files: &ContainerLoadedItemDir,
        weapon_name: &str,
        weapon_part: &str,
        projectile_count: usize,
        muzzle_count: usize,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            weapon: LoadWeapon::load_weapon(
                graphics_mt,
                sound_mt,
                files,
                default_files,
                weapon_name,
                weapon_part,
                projectile_count,
                muzzle_count,
            )?,

            bounces: {
                let mut sounds = Vec::new();
                for i in 0..3 {
                    sounds.push(load_sound_file_part_and_upload(
                        sound_mt,
                        files,
                        default_files,
                        weapon_name,
                        &[weapon_part],
                        &format!("bounce{}", i + 1),
                    )?);
                }
                sounds.try_into().unwrap()
            },

            spawn: load_sound_file_part_and_upload(
                sound_mt,
                files,
                default_files,
                weapon_name,
                &[weapon_part],
                "spawn",
            )?,

            collect: load_sound_file_part_and_upload(
                sound_mt,
                files,
                default_files,
                weapon_name,
                &[weapon_part],
                "collect",
            )?,

            heads: {
                let mut texs: Vec<ContainerItemLoadData> = Default::default();
                for splat in 0..3 {
                    texs.push(load_file_part_and_upload(
                        graphics_mt,
                        files,
                        default_files,
                        weapon_name,
                        &[weapon_part],
                        &format!("splat{}", splat + 1),
                    )?);
                }
                texs.try_into().unwrap()
            },
        })
    }
}

#[derive(Debug)]
pub struct LoadShotgun {
    weapon: LoadWeapon,

    spawn: SoundBackendMemory,
    collect: SoundBackendMemory,
}

impl LoadShotgun {
    pub fn load_weapon(
        graphics_mt: &GraphicsMultiThreaded,
        sound_mt: &SoundMultiThreaded,
        files: &ContainerLoadedItemDir,
        default_files: &ContainerLoadedItemDir,
        weapon_name: &str,
        weapon_part: &str,
        projectile_count: usize,
        muzzle_count: usize,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            weapon: LoadWeapon::load_weapon(
                graphics_mt,
                sound_mt,
                files,
                default_files,
                weapon_name,
                weapon_part,
                projectile_count,
                muzzle_count,
            )?,

            spawn: load_sound_file_part_and_upload(
                sound_mt,
                files,
                default_files,
                weapon_name,
                &[weapon_part],
                "spawn",
            )?,

            collect: load_sound_file_part_and_upload(
                sound_mt,
                files,
                default_files,
                weapon_name,
                &[weapon_part],
                "collect",
            )?,
        })
    }
}

#[derive(Debug)]
pub struct LoadHammer {
    weapon: LoadWeapon,

    hits: [SoundBackendMemory; 3],
}

impl LoadHammer {
    pub fn load_weapon(
        graphics_mt: &GraphicsMultiThreaded,
        sound_mt: &SoundMultiThreaded,
        files: &ContainerLoadedItemDir,
        default_files: &ContainerLoadedItemDir,
        weapon_name: &str,
        weapon_part: &str,
        projectile_count: usize,
        muzzle_count: usize,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            weapon: LoadWeapon::load_weapon(
                graphics_mt,
                sound_mt,
                files,
                default_files,
                weapon_name,
                weapon_part,
                projectile_count,
                muzzle_count,
            )?,

            hits: {
                let mut sounds = Vec::new();
                for i in 0..3 {
                    sounds.push(load_sound_file_part_and_upload(
                        sound_mt,
                        files,
                        default_files,
                        weapon_name,
                        &[weapon_part],
                        &format!("hit{}", i + 1),
                    )?);
                }
                sounds.try_into().unwrap()
            },
        })
    }
}

#[derive(Debug)]
pub struct LoadWeapons {
    hammer: LoadHammer,
    gun: LoadWeapon,
    shotgun: LoadShotgun,
    grenade: LoadGrenade,
    laser: LoadLaser,

    weapon_name: String,
}

impl LoadWeapons {
    pub fn load_weapon(
        graphics_mt: &GraphicsMultiThreaded,
        sound_mt: &SoundMultiThreaded,
        files: ContainerLoadedItemDir,
        default_files: &ContainerLoadedItemDir,
        weapon_name: &str,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            // hammer
            hammer: LoadHammer::load_weapon(
                graphics_mt,
                sound_mt,
                &files,
                default_files,
                weapon_name,
                "hammer",
                0,
                0,
            )?,
            // gun
            gun: LoadWeapon::load_weapon(
                graphics_mt,
                sound_mt,
                &files,
                default_files,
                weapon_name,
                "gun",
                1,
                3,
            )?,
            // shotgun
            shotgun: LoadShotgun::load_weapon(
                graphics_mt,
                sound_mt,
                &files,
                default_files,
                weapon_name,
                "shotgun",
                1,
                3,
            )?,
            // grenade
            grenade: LoadGrenade::load_weapon(
                graphics_mt,
                sound_mt,
                &files,
                default_files,
                weapon_name,
                "grenade",
                1,
                0,
            )?,
            // laser
            laser: LoadLaser::load_weapon(
                graphics_mt,
                sound_mt,
                &files,
                default_files,
                weapon_name,
                "laser",
                1,
                0,
            )?,

            weapon_name: weapon_name.to_string(),
        })
    }

    fn load_files_into_textures(
        self,
        texture_handle: &GraphicsTextureHandle,
        sound_object_handle: &SoundObjectHandle,
    ) -> Weapons {
        Weapons {
            hammer: Hammer {
                weapon: LoadWeapon::load_files_into_objects(
                    texture_handle,
                    sound_object_handle,
                    self.hammer.weapon,
                    &self.weapon_name,
                ),
                hits: self
                    .hammer
                    .hits
                    .into_iter()
                    .map(|hammer| sound_object_handle.create(hammer))
                    .collect::<Vec<_>>()
                    .try_into()
                    .unwrap(),
            },
            gun: LoadWeapon::load_files_into_objects(
                texture_handle,
                sound_object_handle,
                self.gun,
                &self.weapon_name,
            ),
            shotgun: Shotgun {
                weapon: LoadWeapon::load_files_into_objects(
                    texture_handle,
                    sound_object_handle,
                    self.shotgun.weapon,
                    &self.weapon_name,
                ),
                spawn: sound_object_handle.create(self.shotgun.spawn),
                collect: sound_object_handle.create(self.shotgun.collect),
            },
            grenade: Grenade {
                weapon: LoadWeapon::load_files_into_objects(
                    texture_handle,
                    sound_object_handle,
                    self.grenade.weapon,
                    &self.weapon_name,
                ),
                spawn: sound_object_handle.create(self.grenade.spawn),
                collect: sound_object_handle.create(self.grenade.collect),
                explosions: self
                    .grenade
                    .explosions
                    .into_iter()
                    .map(|bounce| sound_object_handle.create(bounce))
                    .collect::<Vec<_>>()
                    .try_into()
                    .unwrap(),
            },
            laser: Laser {
                weapon: LoadWeapon::load_files_into_objects(
                    texture_handle,
                    sound_object_handle,
                    self.laser.weapon,
                    &self.weapon_name,
                ),
                spawn: sound_object_handle.create(self.laser.spawn),
                collect: sound_object_handle.create(self.laser.collect),
                bounces: self
                    .laser
                    .bounces
                    .into_iter()
                    .map(|bounce| sound_object_handle.create(bounce))
                    .collect::<Vec<_>>()
                    .try_into()
                    .unwrap(),
                heads: self
                    .laser
                    .heads
                    .into_iter()
                    .map(|head| LoadWeapon::load_file_into_texture(texture_handle, head, "head"))
                    .collect::<Vec<_>>()
                    .try_into()
                    .unwrap(),
            },
        }
    }
}

impl ContainerLoad<Weapons> for LoadWeapons {
    fn load(
        item_name: &str,
        files: ContainerLoadedItem,
        default_files: &ContainerLoadedItemDir,
        _runtime_thread_pool: &Arc<rayon::ThreadPool>,
        graphics_mt: &GraphicsMultiThreaded,
        sound_mt: &SoundMultiThreaded,
    ) -> anyhow::Result<Self> {
        match files {
            ContainerLoadedItem::Directory(files) => {
                Self::load_weapon(graphics_mt, sound_mt, files, default_files, item_name)
            }
            ContainerLoadedItem::SingleFile(_) => Err(anyhow::anyhow!(
                "single file mode is currently not supported"
            )),
        }
    }

    fn convert(
        self,
        texture_handle: &GraphicsTextureHandle,
        sound_object_handle: &SoundObjectHandle,
    ) -> Weapons {
        self.load_files_into_textures(texture_handle, sound_object_handle)
    }
}

pub type WeaponContainer = Container<Weapons, LoadWeapons>;
pub const WEAPON_CONTAINER_PATH: &str = "weapons/";
