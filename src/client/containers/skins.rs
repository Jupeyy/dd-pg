use std::{str::FromStr, sync::Arc};

use arrayvec::{ArrayString, ArrayVec};
use async_trait::async_trait;
use base_fs::filesys::FileSystem;
use graphics::graphics::{Graphics, GraphicsTextureAllocations};
use graphics_types::{
    command_buffer::{TexFlags, TexFormat},
    rendering::TextureIndex,
    types::ImageFormat,
};

use crate::client::image::png::load_png_image;

use super::container::{load_file_part, Container, ContainerItemInterface, ContainerLoad};

enum SkinEye {
    TODO,

    Count = 9,
}

#[derive(Clone)]
pub struct Skin {
    pub body: TextureIndex,
    pub body_outline: TextureIndex,

    pub marking: TextureIndex,
    pub marking_outline: TextureIndex,

    pub decoration: TextureIndex,
    pub decoration_outline: TextureIndex,

    pub left_hand: TextureIndex,
    pub left_hand_outline: TextureIndex,

    pub right_hand: TextureIndex,
    pub right_hand_outline: TextureIndex,

    pub left_foot: TextureIndex,
    pub left_foot_outline: TextureIndex,

    pub right_foot: TextureIndex,
    pub right_foot_outline: TextureIndex,

    pub left_eyes: [TextureIndex; SkinEye::Count as usize],
    pub left_eyes_outline: [TextureIndex; SkinEye::Count as usize],

    pub right_eyes: [TextureIndex; SkinEye::Count as usize],
    pub right_eyes_outline: [TextureIndex; SkinEye::Count as usize],
}

impl ContainerItemInterface for Skin {
    fn destroy(self, graphics: &mut Graphics) {
        graphics.unload_texture(self.body);
        graphics.unload_texture(self.body_outline);
        graphics.unload_texture(self.marking);
        graphics.unload_texture(self.marking_outline);
        graphics.unload_texture(self.decoration);
        graphics.unload_texture(self.decoration_outline);
        graphics.unload_texture(self.left_hand);
        graphics.unload_texture(self.left_hand_outline);
        graphics.unload_texture(self.right_hand);
        graphics.unload_texture(self.right_hand_outline);
        graphics.unload_texture(self.left_foot);
        graphics.unload_texture(self.left_foot_outline);
        graphics.unload_texture(self.right_foot);
        graphics.unload_texture(self.right_foot_outline);
        self.left_eyes
            .into_iter()
            .for_each(|eye| graphics.unload_texture(eye));
        self.left_eyes_outline
            .into_iter()
            .for_each(|eye| graphics.unload_texture(eye));
        self.right_eyes
            .into_iter()
            .for_each(|eye| graphics.unload_texture(eye));
        self.right_eyes_outline
            .into_iter()
            .for_each(|eye| graphics.unload_texture(eye));
    }
}

#[derive(Default, Clone)]
pub struct LoadSkin {
    body: Vec<u8>,
    body_outline: Vec<u8>,

    marking: Vec<u8>,
    marking_outline: Vec<u8>,

    decoration: Vec<u8>,
    decoration_outline: Vec<u8>,

    left_hand: Vec<u8>,
    left_hand_outline: Vec<u8>,

    right_hand: Vec<u8>,
    right_hand_outline: Vec<u8>,

    left_foot: Vec<u8>,
    left_foot_outline: Vec<u8>,

    right_foot: Vec<u8>,
    right_foot_outline: Vec<u8>,

    left_eyes: [Vec<u8>; SkinEye::Count as usize],
    left_eyes_outline: [Vec<u8>; SkinEye::Count as usize],

    right_eyes: [Vec<u8>; SkinEye::Count as usize],
    right_eyes_outline: [Vec<u8>; SkinEye::Count as usize],

    skin_name: String,
}

impl LoadSkin {
    pub async fn load_skin(&mut self, fs: &FileSystem, skin_name: &str) -> anyhow::Result<()> {
        let skin_path = ArrayString::<4096>::from_str("skins/").unwrap();

        // body file
        self.body = load_file_part(fs, &skin_path, skin_name, &[], "body").await?;
        self.body_outline = load_file_part(fs, &skin_path, skin_name, &[], "body_outline").await?;

        // foot_left file
        self.left_foot = load_file_part(fs, &skin_path, skin_name, &[], "foot_left").await?;
        self.left_foot_outline =
            load_file_part(fs, &skin_path, skin_name, &[], "foot_left_outline").await?;

        // foot_right file
        self.right_foot = load_file_part(fs, &skin_path, skin_name, &[], "foot_right").await?;
        self.right_foot_outline =
            load_file_part(fs, &skin_path, skin_name, &[], "foot_right_outline").await?;

        // hand_left file
        self.left_hand = load_file_part(fs, &skin_path, skin_name, &[], "hand_left").await?;
        self.left_hand_outline =
            load_file_part(fs, &skin_path, skin_name, &[], "hand_left_outline").await?;

        // hand_right file
        self.right_hand = load_file_part(fs, &skin_path, skin_name, &[], "hand_right").await?;
        self.right_hand_outline =
            load_file_part(fs, &skin_path, skin_name, &[], "hand_right_outline").await?;

        // eye_left file
        let eye_file = load_file_part(fs, &skin_path, skin_name, &[], "eye_left").await?;
        self.left_eyes.iter_mut().for_each(|eye| {
            *eye = eye_file.clone();
        });
        self.left_eyes_outline.iter_mut().for_each(|eye| {
            *eye = eye_file.clone();
        });

        // eye_right file
        let eye_file = load_file_part(fs, &skin_path, skin_name, &[], "eye_right").await?;
        self.right_eyes.iter_mut().for_each(|eye| {
            *eye = eye_file.clone();
        });
        self.right_eyes_outline.iter_mut().for_each(|eye| {
            *eye = eye_file.clone();
        });

        // decoration file
        self.decoration = load_file_part(fs, &skin_path, skin_name, &[], "decoration").await?;
        self.decoration_outline =
            load_file_part(fs, &skin_path, skin_name, &[], "decoration").await?;

        // marking file
        self.marking = load_file_part(fs, &skin_path, skin_name, &[], "marking").await?;
        self.marking_outline = load_file_part(fs, &skin_path, skin_name, &[], "marking").await?;

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
}

#[async_trait]
impl ContainerLoad<Skin> for LoadSkin {
    async fn load(&mut self, item_name: &str, fs: &Arc<FileSystem>) -> anyhow::Result<()> {
        self.load_skin(fs, item_name).await?;
        self.skin_name = item_name.to_string();
        Ok(())
    }

    fn convert(self, graphics: &mut Graphics) -> Skin {
        Skin {
            body: Self::load_file_into_texture(graphics, &self.body, &self.skin_name),
            body_outline: Self::load_file_into_texture(
                graphics,
                &self.body_outline,
                &self.skin_name,
            ),
            marking: Self::load_file_into_texture(graphics, &self.marking, &self.skin_name),
            marking_outline: Self::load_file_into_texture(
                graphics,
                &self.marking_outline,
                &self.skin_name,
            ),
            decoration: Self::load_file_into_texture(graphics, &self.decoration, &self.skin_name),
            decoration_outline: Self::load_file_into_texture(
                graphics,
                &self.decoration_outline,
                &self.skin_name,
            ),
            left_hand: Self::load_file_into_texture(graphics, &self.left_hand, &self.skin_name),
            left_hand_outline: Self::load_file_into_texture(
                graphics,
                &self.left_hand_outline,
                &self.skin_name,
            ),
            right_hand: Self::load_file_into_texture(graphics, &self.right_hand, &self.skin_name),
            right_hand_outline: Self::load_file_into_texture(
                graphics,
                &self.right_hand_outline,
                &self.skin_name,
            ),
            left_foot: Self::load_file_into_texture(graphics, &self.left_foot, &self.skin_name),
            left_foot_outline: Self::load_file_into_texture(
                graphics,
                &self.left_foot_outline,
                &self.skin_name,
            ),
            right_foot: Self::load_file_into_texture(graphics, &self.right_foot, &self.skin_name),
            right_foot_outline: Self::load_file_into_texture(
                graphics,
                &self.right_foot_outline,
                &self.skin_name,
            ),
            left_eyes: self
                .left_eyes
                .iter()
                .map(|eye| Self::load_file_into_texture(graphics, eye, &self.skin_name))
                .collect::<ArrayVec<_, 9>>()
                .into_inner()
                .unwrap(),
            left_eyes_outline: self
                .left_eyes_outline
                .iter()
                .map(|eye| Self::load_file_into_texture(graphics, eye, &self.skin_name))
                .collect::<ArrayVec<_, 9>>()
                .into_inner()
                .unwrap(),
            right_eyes: self
                .right_eyes
                .iter()
                .map(|eye| Self::load_file_into_texture(graphics, &eye, &self.skin_name))
                .collect::<ArrayVec<_, 9>>()
                .into_inner()
                .unwrap(),
            right_eyes_outline: self
                .right_eyes_outline
                .iter()
                .map(|eye| Self::load_file_into_texture(graphics, &eye, &self.skin_name))
                .collect::<ArrayVec<_, 9>>()
                .into_inner()
                .unwrap(),
        }
    }
}

pub type SkinContainer = Container<Skin, LoadSkin>;
