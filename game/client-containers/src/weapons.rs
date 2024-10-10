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
use hiarc::Hiarc;
use sound::{
    sound_handle::SoundObjectHandle, sound_mt::SoundMultiThreaded,
    sound_mt_types::SoundBackendMemory, sound_object::SoundObject,
};

use crate::container::{
    load_file_part_and_upload_ex, load_sound_file_part_and_upload,
    load_sound_file_part_and_upload_ex, ContainerLoadedItem, ContainerLoadedItemDir,
};

use super::container::{
    load_file_part_and_upload, Container, ContainerItemLoadData, ContainerLoad,
};

#[derive(Debug, Hiarc, Clone)]
pub struct Weapon {
    pub tex: TextureContainer,
    pub cursor: TextureContainer,
    pub projectiles: Vec<TextureContainer>,
    pub muzzles: Vec<TextureContainer>,

    pub fire: Vec<SoundObject>,
    pub switch: Vec<SoundObject>,
    pub noammo: Vec<SoundObject>,
}

#[derive(Debug, Hiarc, Clone)]
pub struct Grenade {
    pub weapon: Weapon,
    pub spawn: SoundObject,
    pub collect: SoundObject,
    pub explosions: Vec<SoundObject>,
}

#[derive(Debug, Hiarc, Clone)]
pub struct Laser {
    pub weapon: Weapon,
    pub spawn: SoundObject,
    pub collect: SoundObject,
    pub bounces: Vec<SoundObject>,
    pub heads: Vec<TextureContainer>,
}

#[derive(Debug, Hiarc, Clone)]
pub struct Shotgun {
    pub weapon: Weapon,
    pub spawn: SoundObject,
    pub collect: SoundObject,
}

#[derive(Debug, Hiarc, Clone)]
pub struct Hammer {
    pub weapon: Weapon,
    pub hits: Vec<SoundObject>,
}

#[derive(Debug, Hiarc, Clone)]
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

#[derive(Debug, Hiarc)]
pub struct LoadWeapon {
    tex: ContainerItemLoadData,
    cursor_tex: ContainerItemLoadData,
    projectiles: Vec<ContainerItemLoadData>,
    muzzles: Vec<ContainerItemLoadData>,

    fire: Vec<SoundBackendMemory>,
    switch: Vec<SoundBackendMemory>,
    noammo: Vec<SoundBackendMemory>,
}

impl LoadWeapon {
    pub fn new(
        graphics_mt: &GraphicsMultiThreaded,
        sound_mt: &SoundMultiThreaded,
        files: &ContainerLoadedItemDir,
        default_files: &ContainerLoadedItemDir,
        weapon_name: &str,
        weapon_part: &str,
        has_projectiles: bool,
        has_muzzle: bool,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            // weapon
            tex: load_file_part_and_upload(
                graphics_mt,
                files,
                default_files,
                weapon_name,
                &[weapon_part],
                "weapon",
            )?
            .img,
            // cursor
            cursor_tex: load_file_part_and_upload(
                graphics_mt,
                files,
                default_files,
                weapon_name,
                &[weapon_part],
                "cursor",
            )?
            .img,

            projectiles: {
                let mut textures = Vec::new();
                let mut i = 0;
                let mut allow_default = true;
                if has_projectiles {
                    loop {
                        match load_file_part_and_upload_ex(
                            graphics_mt,
                            files,
                            default_files,
                            weapon_name,
                            &[weapon_part],
                            &format!("projectile{i}"),
                            allow_default,
                        ) {
                            Ok(img) => {
                                allow_default &= img.from_default;
                                textures.push(img.img);
                            }
                            Err(err) => {
                                if i == 0 {
                                    return Err(err);
                                } else {
                                    break;
                                }
                            }
                        }

                        i += 1;
                    }
                }
                textures
            },
            muzzles: {
                let mut textures = Vec::new();
                let mut i = 0;
                let mut allow_default = true;
                if has_muzzle {
                    loop {
                        match load_file_part_and_upload_ex(
                            graphics_mt,
                            files,
                            default_files,
                            weapon_name,
                            &[weapon_part],
                            &format!("muzzle{i}"),
                            allow_default,
                        ) {
                            Ok(img) => {
                                allow_default &= img.from_default;
                                textures.push(img.img);
                            }
                            Err(err) => {
                                if i == 0 {
                                    return Err(err);
                                } else {
                                    break;
                                }
                            }
                        }

                        i += 1;
                    }
                }
                textures
            },

            fire: {
                let mut sounds = Vec::new();
                let mut i = 0;
                let mut allow_default = true;
                loop {
                    match load_sound_file_part_and_upload_ex(
                        sound_mt,
                        files,
                        default_files,
                        weapon_name,
                        &[weapon_part],
                        &format!("fire{}", i + 1),
                        allow_default,
                    ) {
                        Ok(sound) => {
                            allow_default &= sound.from_default;
                            sounds.push(sound.mem);
                        }
                        Err(err) => {
                            if i == 0 {
                                return Err(err);
                            } else {
                                break;
                            }
                        }
                    }
                    i += 1;
                }
                sounds
            },
            switch: {
                let mut sounds = Vec::new();
                let mut i = 0;
                let mut allow_default = true;
                loop {
                    match load_sound_file_part_and_upload_ex(
                        sound_mt,
                        files,
                        default_files,
                        weapon_name,
                        &[weapon_part],
                        &format!("switch{}", i + 1),
                        allow_default,
                    ) {
                        Ok(sound) => {
                            allow_default &= sound.from_default;
                            sounds.push(sound.mem);
                        }
                        Err(err) => {
                            if i == 0 {
                                return Err(err);
                            } else {
                                break;
                            }
                        }
                    }
                    i += 1;
                }
                sounds
            },
            noammo: {
                let mut sounds = Vec::new();
                let mut i = 0;
                let mut allow_default = true;
                loop {
                    match load_sound_file_part_and_upload_ex(
                        sound_mt,
                        files,
                        default_files,
                        weapon_name,
                        &[weapon_part],
                        &format!("noammo{}", i + 1),
                        allow_default,
                    ) {
                        Ok(sound) => {
                            allow_default &= sound.from_default;
                            sounds.push(sound.mem);
                        }
                        Err(err) => {
                            if i == 0 {
                                return Err(err);
                            } else {
                                break;
                            }
                        }
                    }
                    i += 1;
                }
                sounds
            },
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
                TexFormat::Rgba,
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

            fire: file
                .fire
                .into_iter()
                .map(|snd| sound_object_handle.create(snd))
                .collect(),
            switch: file
                .switch
                .into_iter()
                .map(|snd| sound_object_handle.create(snd))
                .collect(),
            noammo: file
                .noammo
                .into_iter()
                .map(|snd| sound_object_handle.create(snd))
                .collect(),
        }
    }
}

#[derive(Debug, Hiarc)]
pub struct LoadGrenade {
    weapon: LoadWeapon,

    spawn: SoundBackendMemory,
    collect: SoundBackendMemory,
    explosions: Vec<SoundBackendMemory>,
}

impl LoadGrenade {
    pub fn new(
        graphics_mt: &GraphicsMultiThreaded,
        sound_mt: &SoundMultiThreaded,
        files: &ContainerLoadedItemDir,
        default_files: &ContainerLoadedItemDir,
        weapon_name: &str,
        weapon_part: &str,
        has_projectile: bool,
        has_muzzle: bool,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            weapon: LoadWeapon::new(
                graphics_mt,
                sound_mt,
                files,
                default_files,
                weapon_name,
                weapon_part,
                has_projectile,
                has_muzzle,
            )?,

            explosions: {
                let mut sounds = Vec::new();
                let mut i = 0;
                let mut allow_default = true;
                loop {
                    match load_sound_file_part_and_upload_ex(
                        sound_mt,
                        files,
                        default_files,
                        weapon_name,
                        &[weapon_part],
                        &format!("explosion{}", i + 1),
                        allow_default,
                    ) {
                        Ok(sound) => {
                            allow_default &= sound.from_default;
                            sounds.push(sound.mem);
                        }
                        Err(err) => {
                            if i == 0 {
                                return Err(err);
                            } else {
                                break;
                            }
                        }
                    }
                    i += 1;
                }
                sounds
            },

            spawn: load_sound_file_part_and_upload(
                sound_mt,
                files,
                default_files,
                weapon_name,
                &[weapon_part],
                "spawn",
            )?
            .mem,

            collect: load_sound_file_part_and_upload(
                sound_mt,
                files,
                default_files,
                weapon_name,
                &[weapon_part],
                "collect",
            )?
            .mem,
        })
    }
}

#[derive(Debug, Hiarc)]
pub struct LoadLaser {
    weapon: LoadWeapon,

    spawn: SoundBackendMemory,
    collect: SoundBackendMemory,
    bounces: Vec<SoundBackendMemory>,
    heads: Vec<ContainerItemLoadData>,
}

impl LoadLaser {
    pub fn new(
        graphics_mt: &GraphicsMultiThreaded,
        sound_mt: &SoundMultiThreaded,
        files: &ContainerLoadedItemDir,
        default_files: &ContainerLoadedItemDir,
        weapon_name: &str,
        weapon_part: &str,
        has_projectile: bool,
        has_muzzle: bool,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            weapon: LoadWeapon::new(
                graphics_mt,
                sound_mt,
                files,
                default_files,
                weapon_name,
                weapon_part,
                has_projectile,
                has_muzzle,
            )?,

            bounces: {
                let mut sounds = Vec::new();
                let mut i = 0;
                let mut allow_default = true;
                loop {
                    match load_sound_file_part_and_upload_ex(
                        sound_mt,
                        files,
                        default_files,
                        weapon_name,
                        &[weapon_part],
                        &format!("bounce{}", i + 1),
                        allow_default,
                    ) {
                        Ok(sound) => {
                            allow_default &= sound.from_default;
                            sounds.push(sound.mem);
                        }
                        Err(err) => {
                            if i == 0 {
                                return Err(err);
                            } else {
                                break;
                            }
                        }
                    }
                    i += 1;
                }
                sounds
            },

            spawn: load_sound_file_part_and_upload(
                sound_mt,
                files,
                default_files,
                weapon_name,
                &[weapon_part],
                "spawn",
            )?
            .mem,

            collect: load_sound_file_part_and_upload(
                sound_mt,
                files,
                default_files,
                weapon_name,
                &[weapon_part],
                "collect",
            )?
            .mem,

            heads: {
                let mut textures: Vec<ContainerItemLoadData> = Default::default();
                let mut i = 0;
                let mut allow_default = true;
                loop {
                    match load_file_part_and_upload_ex(
                        graphics_mt,
                        files,
                        default_files,
                        weapon_name,
                        &[weapon_part],
                        &format!("splat{}", i + 1),
                        allow_default,
                    ) {
                        Ok(img) => {
                            allow_default &= img.from_default;
                            textures.push(img.img);
                        }
                        Err(err) => {
                            if i == 0 {
                                return Err(err);
                            } else {
                                break;
                            }
                        }
                    }

                    i += 1;
                }
                textures
            },
        })
    }
}

#[derive(Debug, Hiarc)]
pub struct LoadShotgun {
    weapon: LoadWeapon,

    spawn: SoundBackendMemory,
    collect: SoundBackendMemory,
}

impl LoadShotgun {
    pub fn new(
        graphics_mt: &GraphicsMultiThreaded,
        sound_mt: &SoundMultiThreaded,
        files: &ContainerLoadedItemDir,
        default_files: &ContainerLoadedItemDir,
        weapon_name: &str,
        weapon_part: &str,
        has_projectiles: bool,
        has_muzzle: bool,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            weapon: LoadWeapon::new(
                graphics_mt,
                sound_mt,
                files,
                default_files,
                weapon_name,
                weapon_part,
                has_projectiles,
                has_muzzle,
            )?,

            spawn: load_sound_file_part_and_upload(
                sound_mt,
                files,
                default_files,
                weapon_name,
                &[weapon_part],
                "spawn",
            )?
            .mem,

            collect: load_sound_file_part_and_upload(
                sound_mt,
                files,
                default_files,
                weapon_name,
                &[weapon_part],
                "collect",
            )?
            .mem,
        })
    }
}

#[derive(Debug, Hiarc)]
pub struct LoadHammer {
    weapon: LoadWeapon,

    hits: Vec<SoundBackendMemory>,
}

impl LoadHammer {
    pub fn new(
        graphics_mt: &GraphicsMultiThreaded,
        sound_mt: &SoundMultiThreaded,
        files: &ContainerLoadedItemDir,
        default_files: &ContainerLoadedItemDir,
        weapon_name: &str,
        weapon_part: &str,
        has_projectile: bool,
        has_muzzle: bool,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            weapon: LoadWeapon::new(
                graphics_mt,
                sound_mt,
                files,
                default_files,
                weapon_name,
                weapon_part,
                has_projectile,
                has_muzzle,
            )?,

            hits: {
                let mut sounds = Vec::new();
                let mut i = 0;
                let mut allow_default = true;
                loop {
                    match load_sound_file_part_and_upload_ex(
                        sound_mt,
                        files,
                        default_files,
                        weapon_name,
                        &[weapon_part],
                        &format!("hit{}", i + 1),
                        allow_default,
                    ) {
                        Ok(sound) => {
                            allow_default &= sound.from_default;
                            sounds.push(sound.mem);
                        }
                        Err(err) => {
                            if i == 0 {
                                return Err(err);
                            } else {
                                break;
                            }
                        }
                    }
                    i += 1;
                }
                sounds
            },
        })
    }
}

#[derive(Debug, Hiarc)]
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
            hammer: LoadHammer::new(
                graphics_mt,
                sound_mt,
                &files,
                default_files,
                weapon_name,
                "hammer",
                false,
                false,
            )?,
            // gun
            gun: LoadWeapon::new(
                graphics_mt,
                sound_mt,
                &files,
                default_files,
                weapon_name,
                "gun",
                true,
                true,
            )?,
            // shotgun
            shotgun: LoadShotgun::new(
                graphics_mt,
                sound_mt,
                &files,
                default_files,
                weapon_name,
                "shotgun",
                true,
                true,
            )?,
            // grenade
            grenade: LoadGrenade::new(
                graphics_mt,
                sound_mt,
                &files,
                default_files,
                weapon_name,
                "grenade",
                true,
                false,
            )?,
            // laser
            laser: LoadLaser::new(
                graphics_mt,
                sound_mt,
                &files,
                default_files,
                weapon_name,
                "laser",
                true,
                false,
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
                    .collect::<Vec<_>>(),
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
                    .collect::<Vec<_>>(),
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
                    .collect::<Vec<_>>(),
                heads: self
                    .laser
                    .heads
                    .into_iter()
                    .map(|head| LoadWeapon::load_file_into_texture(texture_handle, head, "head"))
                    .collect::<Vec<_>>(),
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
