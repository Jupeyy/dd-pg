use std::{str::FromStr, sync::Arc};

use arrayvec::ArrayString;
use async_trait::async_trait;
use base_fs::filesys::FileSystem;

use graphics_backend::types::Graphics;
use graphics_types::{
    command_buffer::{TexFlags, TexFormat},
    textures_handle::{GraphicsTextureHandleInterface, TextureIndex},
    types::ImageFormat,
};
use image::png::load_png_image;

use super::container::{load_file_part, Container, ContainerLoad};

#[derive(Clone)]
pub struct Weapon {
    pub tex: TextureIndex,
    pub cursor: TextureIndex,
    pub projectiles: Vec<TextureIndex>,
}

#[derive(Clone)]
pub struct Weapons {
    pub gun: Weapon,
    pub shotgun: Weapon,
}

#[derive(Default, Clone)]
pub struct LoadWeapon {
    tex: Vec<u8>,
    cursor_tex: Vec<u8>,
    projectiles: Vec<Vec<u8>>,
}

impl LoadWeapon {
    pub async fn load_weapon(
        &mut self,
        fs: &FileSystem,
        weapon_name: &str,
        weapon_part: &str,
        projectile_count: usize,
    ) -> anyhow::Result<()> {
        let weapon_path = ArrayString::<4096>::from_str("weapons/").unwrap();

        // weapon
        self.tex = load_file_part(fs, &weapon_path, weapon_name, &[weapon_part], "weapon").await?;
        // cursor
        self.cursor_tex =
            load_file_part(fs, &weapon_path, weapon_name, &[weapon_part], "cursor").await?;
        // projectiles
        for proj in 0..projectile_count {
            self.projectiles.push(
                load_file_part(
                    fs,
                    &weapon_path,
                    weapon_name,
                    &[weapon_part],
                    &("projectile".to_string() + &proj.to_string()),
                )
                .await?,
            );
        }

        Ok(())
    }

    fn load_file_into_texture(graphics: &mut Graphics, file: &Vec<u8>, name: &str) -> TextureIndex {
        let mut img_data = Vec::<u8>::new();
        let part_img = load_png_image(&file, |size| {
            img_data = vec![0; size];
            &mut img_data
        })
        .unwrap();
        let texture_id = graphics
            .texture_handle
            .load_texture_slow(
                part_img.width as usize,
                part_img.height as usize,
                ImageFormat::Rgba as i32,
                part_img.data.to_vec(),
                TexFormat::RGBA as i32,
                TexFlags::empty(),
                name,
            )
            .unwrap();
        texture_id
    }

    fn load_files_into_textures(graphics: &mut Graphics, file: &LoadWeapon, name: &str) -> Weapon {
        Weapon {
            tex: Self::load_file_into_texture(graphics, &file.tex, name),
            cursor: Self::load_file_into_texture(graphics, &file.cursor_tex, name),
            projectiles: file
                .projectiles
                .iter()
                .map(|file| Self::load_file_into_texture(graphics, file, name))
                .collect(),
        }
    }
}

#[derive(Default, Clone)]
pub struct LoadWeapons {
    gun: LoadWeapon,
    shotgun: LoadWeapon,

    weapon_name: String,
}

impl LoadWeapons {
    pub async fn load_weapon(&mut self, fs: &FileSystem, weapon_name: &str) -> anyhow::Result<()> {
        // gun
        self.gun.load_weapon(fs, weapon_name, "gun", 1).await?;
        // shotgun
        self.shotgun
            .load_weapon(fs, weapon_name, "shotgun", 1)
            .await?;

        Ok(())
    }

    fn load_files_into_textures(self, graphics: &mut Graphics) -> Weapons {
        Weapons {
            gun: LoadWeapon::load_files_into_textures(graphics, &self.gun, &self.weapon_name),
            shotgun: LoadWeapon::load_files_into_textures(
                graphics,
                &self.shotgun,
                &self.weapon_name,
            ),
        }
    }
}

#[async_trait]
impl ContainerLoad<Weapons> for LoadWeapons {
    async fn load(
        &mut self,
        item_name: &str,
        fs: &Arc<FileSystem>,
        _runtime_thread_pool: &Arc<rayon::ThreadPool>,
    ) -> anyhow::Result<()> {
        self.load_weapon(fs, item_name).await?;
        self.weapon_name = item_name.to_string();
        Ok(())
    }

    fn convert(self, graphics: &mut Graphics) -> Weapons {
        self.load_files_into_textures(graphics)
    }
}

pub type WeaponContainer = Container<Weapons, LoadWeapons>;
