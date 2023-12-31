use std::{str::FromStr, sync::Arc};

use arrayvec::ArrayString;
use async_trait::async_trait;

use base_io_traits::fs_traits::FileSystemInterface;
use graphics::{graphics::GraphicsTextureHandle, graphics_mt::GraphicsMultiThreaded};
use graphics_types::{
    commands::{TexFlags, TexFormat},
    textures_handle::TextureIndex,
    types::ImageFormat,
};

use super::container::{
    load_file_part_and_upload, Container, ContainerItemLoadData, ContainerLoad,
};

#[derive(Clone)]
pub struct Weapon {
    pub tex: TextureIndex,
    pub cursor: TextureIndex,
    pub projectiles: Vec<TextureIndex>,
    pub muzzles: Vec<TextureIndex>,
}

#[derive(Clone)]
pub struct Weapons {
    pub hammer: Weapon,
    pub gun: Weapon,
    pub shotgun: Weapon,
    pub grenade: Weapon,
    pub laser: Weapon,
}

#[derive(Debug)]
pub struct LoadWeapon {
    tex: ContainerItemLoadData,
    cursor_tex: ContainerItemLoadData,
    projectiles: Vec<ContainerItemLoadData>,
    muzzles: Vec<ContainerItemLoadData>,
}

impl LoadWeapon {
    pub async fn load_weapon(
        graphics_mt: &GraphicsMultiThreaded,
        fs: &dyn FileSystemInterface,
        weapon_name: &str,
        weapon_part: &str,
        projectile_count: usize,
        muzzle_count: usize,
    ) -> anyhow::Result<Self> {
        let weapon_path = ArrayString::<4096>::from_str("weapons/").unwrap();

        let mut projectiles: Vec<ContainerItemLoadData> = Default::default();
        let mut muzzles: Vec<ContainerItemLoadData> = Default::default();
        // projectiles
        for proj in 0..projectile_count {
            projectiles.push(
                load_file_part_and_upload(
                    graphics_mt,
                    fs,
                    &weapon_path,
                    weapon_name,
                    &[weapon_part],
                    &("projectile".to_string() + &proj.to_string()),
                )
                .await?,
            );
        }
        // muzzles
        for muzzle in 0..muzzle_count {
            muzzles.push(
                load_file_part_and_upload(
                    graphics_mt,
                    fs,
                    &weapon_path,
                    weapon_name,
                    &[weapon_part],
                    &("muzzle".to_string() + &muzzle.to_string()),
                )
                .await?,
            );
        }

        Ok(Self {
            // weapon
            tex: load_file_part_and_upload(
                graphics_mt,
                fs,
                &weapon_path,
                weapon_name,
                &[weapon_part],
                "weapon",
            )
            .await?,
            // cursor
            cursor_tex: load_file_part_and_upload(
                graphics_mt,
                fs,
                &weapon_path,
                weapon_name,
                &[weapon_part],
                "cursor",
            )
            .await?,

            projectiles,
            muzzles,
        })
    }

    fn load_file_into_texture(
        texture_handle: &GraphicsTextureHandle,
        img: ContainerItemLoadData,
        name: &str,
    ) -> TextureIndex {
        texture_handle
            .load_texture(
                img.width as usize,
                img.height as usize,
                ImageFormat::Rgba as i32,
                img.data,
                TexFormat::RGBA as i32,
                TexFlags::empty(),
                name,
            )
            .unwrap()
    }

    fn load_files_into_textures(
        texture_handle: &GraphicsTextureHandle,
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
        }
    }
}

#[derive(Debug)]
pub struct LoadWeapons {
    hammer: LoadWeapon,
    gun: LoadWeapon,
    shotgun: LoadWeapon,
    grenade: LoadWeapon,
    laser: LoadWeapon,

    weapon_name: String,
}

impl LoadWeapons {
    pub async fn load_weapon(
        graphics_mt: &GraphicsMultiThreaded,
        fs: &dyn FileSystemInterface,
        weapon_name: &str,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            // hammer
            hammer: LoadWeapon::load_weapon(graphics_mt, fs, weapon_name, "hammer", 0, 0).await?,
            // gun
            gun: LoadWeapon::load_weapon(graphics_mt, fs, weapon_name, "gun", 1, 3).await?,
            // shotgun
            shotgun: LoadWeapon::load_weapon(graphics_mt, fs, weapon_name, "shotgun", 1, 3).await?,
            // grenade
            grenade: LoadWeapon::load_weapon(graphics_mt, fs, weapon_name, "grenade", 1, 0).await?,
            // laser
            laser: LoadWeapon::load_weapon(graphics_mt, fs, weapon_name, "laser", 1, 0).await?,

            weapon_name: weapon_name.to_string(),
        })
    }

    fn load_files_into_textures(self, texture_handle: &GraphicsTextureHandle) -> Weapons {
        Weapons {
            hammer: LoadWeapon::load_files_into_textures(
                texture_handle,
                self.hammer,
                &self.weapon_name,
            ),
            gun: LoadWeapon::load_files_into_textures(texture_handle, self.gun, &self.weapon_name),
            shotgun: LoadWeapon::load_files_into_textures(
                texture_handle,
                self.shotgun,
                &self.weapon_name,
            ),
            grenade: LoadWeapon::load_files_into_textures(
                texture_handle,
                self.grenade,
                &self.weapon_name,
            ),
            laser: LoadWeapon::load_files_into_textures(
                texture_handle,
                self.laser,
                &self.weapon_name,
            ),
        }
    }
}

#[async_trait]
impl ContainerLoad<Weapons> for LoadWeapons {
    async fn load(
        item_name: &str,
        fs: &Arc<dyn FileSystemInterface>,
        _runtime_thread_pool: &Arc<rayon::ThreadPool>,
        graphics_mt: &GraphicsMultiThreaded,
    ) -> anyhow::Result<Self> {
        Self::load_weapon(graphics_mt, fs.as_ref(), item_name).await
    }

    fn convert(self, texture_handle: &GraphicsTextureHandle) -> Weapons {
        self.load_files_into_textures(texture_handle)
    }
}

pub type WeaponContainer = Container<Weapons, LoadWeapons>;
