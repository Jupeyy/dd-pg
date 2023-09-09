use std::{future::Future, pin::Pin, str::FromStr, sync::Arc};

use arrayvec::{ArrayString, ArrayVec};
use async_trait::async_trait;
use base_fs::filesys::FileSystem;

use fixed::{types::extra::U32, FixedI64};
use graphics_backend::types::Graphics;
use graphics_types::{
    command_buffer::{TexFlags, TexFormat},
    rendering::ColorRGBA,
    textures_handle::{GraphicsTextureHandleInterface, TextureIndex},
    types::ImageFormat,
};
use image::png::PngResultPersistent;
use math::math::vector::vec3;

use super::container::{load_file_part_as_png, Container, ContainerLoad};

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum TeeSkinEye {
    Normal = 0,
    Angry,
    Pain,
    Happy,
    Dead,
    Surprised,
    Blink,

    Count = 7,
}

#[derive(Debug, Clone)]
pub struct SkinMetricVariable {
    min_x: FixedI64<U32>,
    min_y: FixedI64<U32>,
    max_x: FixedI64<U32>,
    max_y: FixedI64<U32>,
}

impl Default for SkinMetricVariable {
    fn default() -> Self {
        Self {
            min_x: FixedI64::<U32>::MAX,
            min_y: FixedI64::<U32>::MAX,
            max_x: FixedI64::<U32>::MIN,
            max_y: FixedI64::<U32>::MIN,
        }
    }
}

impl SkinMetricVariable {
    // bb = bounding box
    fn width_bb(&self) -> FixedI64<U32> {
        self.max_x - self.min_x
    }
    fn height_bb(&self) -> FixedI64<U32> {
        self.max_y - self.min_y
    }

    pub fn width(&self) -> FixedI64<U32> {
        self.width_bb()
    }

    pub fn height(&self) -> FixedI64<U32> {
        self.height_bb()
    }

    pub fn x(&self) -> FixedI64<U32> {
        self.min_x
    }

    pub fn y(&self) -> FixedI64<U32> {
        self.min_y
    }

    pub fn from_texture(
        &mut self,
        img: &[u8],
        img_width: u32,
        img_x: u32,
        img_y: u32,
        check_width: u32,
        check_height: u32,
    ) {
        let mut max_y = 0;
        let mut min_y = check_height + 1;
        let mut max_x = 0;
        let mut min_x = check_width + 1;

        for y in 0..check_height {
            for x in 0..check_width {
                let offset_alpha = (y + img_y) * img_width + (x + img_x) * 4 + 3;
                let alpha_value = img[offset_alpha as usize];
                if alpha_value > 0 {
                    max_y = max_y.max(y);
                    min_y = min_y.min(y);
                    max_x = max_x.max(x);
                    min_x = min_x.min(x);
                }
            }
        }

        self.min_x = self
            .min_x
            .min(FixedI64::<U32>::from_num(min_x) / FixedI64::<U32>::from_num(check_width));
        self.min_y = self
            .min_y
            .min(FixedI64::<U32>::from_num(min_y) / FixedI64::<U32>::from_num(check_height));
        self.max_x = self
            .max_x
            .min(FixedI64::<U32>::from_num(max_x) / FixedI64::<U32>::from_num(check_width));
        self.max_y = self
            .max_y
            .min(FixedI64::<U32>::from_num(max_y) / FixedI64::<U32>::from_num(check_height));
    }
}

#[derive(Debug, Default, Clone)]
pub struct SkinMetrics {
    pub body: SkinMetricVariable,
    pub feet: SkinMetricVariable,
}

pub struct SkinTextures {
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

    pub left_eyes: [TextureIndex; TeeSkinEye::Count as usize],
    pub right_eyes: [TextureIndex; TeeSkinEye::Count as usize],
}

pub struct Skin {
    pub textures: SkinTextures,
    pub grey_scaled_textures: SkinTextures,
    pub metrics: SkinMetrics,
}

#[derive(Default, Clone)]
pub struct LoadSkinTextures {
    body: PngResultPersistent,
    body_outline: PngResultPersistent,

    marking: PngResultPersistent,
    marking_outline: PngResultPersistent,

    decoration: PngResultPersistent,
    decoration_outline: PngResultPersistent,

    left_hand: PngResultPersistent,
    left_hand_outline: PngResultPersistent,

    right_hand: PngResultPersistent,
    right_hand_outline: PngResultPersistent,

    left_foot: PngResultPersistent,
    left_foot_outline: PngResultPersistent,

    right_foot: PngResultPersistent,
    right_foot_outline: PngResultPersistent,

    left_eyes: [PngResultPersistent; TeeSkinEye::Count as usize],
    right_eyes: [PngResultPersistent; TeeSkinEye::Count as usize],
}

impl LoadSkinTextures {
    async fn load_skin(&mut self, fs: &FileSystem, skin_name: &str) -> anyhow::Result<()> {
        let skin_path = ArrayString::<4096>::from_str("skins/").unwrap();

        // body file
        self.body = load_file_part_as_png(fs, &skin_path, skin_name, &[], "body").await?;
        self.body_outline =
            load_file_part_as_png(fs, &skin_path, skin_name, &[], "body_outline").await?;

        // foot_left file
        self.left_foot = load_file_part_as_png(fs, &skin_path, skin_name, &[], "foot_left").await?;
        self.left_foot_outline =
            load_file_part_as_png(fs, &skin_path, skin_name, &[], "foot_left_outline").await?;

        // foot_right file
        self.right_foot =
            load_file_part_as_png(fs, &skin_path, skin_name, &[], "foot_right").await?;
        self.right_foot_outline =
            load_file_part_as_png(fs, &skin_path, skin_name, &[], "foot_right_outline").await?;

        // hand_left file
        self.left_hand = load_file_part_as_png(fs, &skin_path, skin_name, &[], "hand_left").await?;
        self.left_hand_outline =
            load_file_part_as_png(fs, &skin_path, skin_name, &[], "hand_left_outline").await?;

        // hand_right file
        self.right_hand =
            load_file_part_as_png(fs, &skin_path, skin_name, &[], "hand_right").await?;
        self.right_hand_outline =
            load_file_part_as_png(fs, &skin_path, skin_name, &[], "hand_right_outline").await?;

        // eyes file
        let load_eyes = |eye_name: &'static str| -> Pin<
            Box<
                dyn Future<
                        Output = anyhow::Result<[PngResultPersistent; TeeSkinEye::Count as usize]>,
                    > + Send,
            >,
        > {
            Box::pin(async {
                let mut eyes: [PngResultPersistent; TeeSkinEye::Count as usize] =
                    Default::default();
                eyes[TeeSkinEye::Angry as usize] =
                    load_file_part_as_png(fs, &skin_path, skin_name, &[eye_name], "angry").await?;
                eyes[TeeSkinEye::Dead as usize] =
                    load_file_part_as_png(fs, &skin_path, skin_name, &[eye_name], "dead").await?;
                eyes[TeeSkinEye::Happy as usize] =
                    load_file_part_as_png(fs, &skin_path, skin_name, &[eye_name], "happy").await?;
                eyes[TeeSkinEye::Normal as usize] =
                    load_file_part_as_png(fs, &skin_path, skin_name, &[eye_name], "normal").await?;
                eyes[TeeSkinEye::Blink as usize] =
                    load_file_part_as_png(fs, &skin_path, skin_name, &[eye_name], "normal").await?; // TODO: wrong
                eyes[TeeSkinEye::Pain as usize] =
                    load_file_part_as_png(fs, &skin_path, skin_name, &[eye_name], "pain").await?;
                eyes[TeeSkinEye::Surprised as usize] =
                    load_file_part_as_png(fs, &skin_path, skin_name, &[eye_name], "surprised")
                        .await?;
                Ok(eyes)
            })
        };
        self.left_eyes = load_eyes("eyes_left").await?;
        self.right_eyes = load_eyes("eyes_right").await?;

        // decoration file
        self.decoration =
            load_file_part_as_png(fs, &skin_path, skin_name, &[], "decoration").await?;
        self.decoration_outline =
            load_file_part_as_png(fs, &skin_path, skin_name, &[], "decoration").await?;

        // marking file
        self.marking = load_file_part_as_png(fs, &skin_path, skin_name, &[], "marking").await?;
        self.marking_outline =
            load_file_part_as_png(fs, &skin_path, skin_name, &[], "marking").await?;

        Ok(())
    }

    fn to_skin(self, skin_name: &str, graphics: &mut Graphics) -> SkinTextures {
        SkinTextures {
            body: LoadSkin::load_file_into_texture(graphics, self.body, skin_name),
            body_outline: LoadSkin::load_file_into_texture(graphics, self.body_outline, skin_name),
            marking: LoadSkin::load_file_into_texture(graphics, self.marking, skin_name),
            marking_outline: LoadSkin::load_file_into_texture(
                graphics,
                self.marking_outline,
                skin_name,
            ),
            decoration: LoadSkin::load_file_into_texture(graphics, self.decoration, skin_name),
            decoration_outline: LoadSkin::load_file_into_texture(
                graphics,
                self.decoration_outline,
                skin_name,
            ),
            left_hand: LoadSkin::load_file_into_texture(graphics, self.left_hand, skin_name),
            left_hand_outline: LoadSkin::load_file_into_texture(
                graphics,
                self.left_hand_outline,
                skin_name,
            ),
            right_hand: LoadSkin::load_file_into_texture(graphics, self.right_hand, skin_name),
            right_hand_outline: LoadSkin::load_file_into_texture(
                graphics,
                self.right_hand_outline,
                skin_name,
            ),
            left_foot: LoadSkin::load_file_into_texture(graphics, self.left_foot, skin_name),
            left_foot_outline: LoadSkin::load_file_into_texture(
                graphics,
                self.left_foot_outline,
                skin_name,
            ),
            right_foot: LoadSkin::load_file_into_texture(graphics, self.right_foot, skin_name),
            right_foot_outline: LoadSkin::load_file_into_texture(
                graphics,
                self.right_foot_outline,
                skin_name,
            ),
            left_eyes: self
                .left_eyes
                .iter()
                .map(|eye| LoadSkin::load_file_into_texture(graphics, eye.clone(), skin_name))
                .collect::<ArrayVec<_, { TeeSkinEye::Count as usize }>>()
                .into_inner()
                .unwrap(),
            right_eyes: self
                .right_eyes
                .iter()
                .map(|eye| LoadSkin::load_file_into_texture(graphics, eye.clone(), skin_name))
                .collect::<ArrayVec<_, { TeeSkinEye::Count as usize }>>()
                .into_inner()
                .unwrap(),
        }
    }
}

#[derive(Default, Clone)]
pub struct LoadSkin {
    textures: LoadSkinTextures,
    grey_scaled_textures: LoadSkinTextures,

    blood_color: ColorRGBA,

    metrics: SkinMetrics,

    skin_name: String,
}

impl LoadSkin {
    fn get_blood_color(body_img: &[u8], body_width: usize, body_height: usize) -> ColorRGBA {
        let pixel_step = 4;

        // dig out blood color
        let mut colors: [i32; 3] = Default::default();
        for y in 0..body_height {
            for x in 0..body_width {
                let alpha_value = body_img[y + x * pixel_step + 3];
                if alpha_value > 128 {
                    colors[0] += body_img[y + x * pixel_step + 0] as i32;
                    colors[1] += body_img[y + x * pixel_step + 1] as i32;
                    colors[2] += body_img[y + x * pixel_step + 2] as i32;
                }
            }
        }
        if colors[0] != 0 && colors[1] != 0 && colors[2] != 0 {
            let color = vec3 {
                x: colors[0] as f32,
                y: colors[1] as f32,
                z: colors[2] as f32,
            }
            .normalize();
            ColorRGBA::new(color.x, color.y, color.z, 1.0)
        } else {
            ColorRGBA::new(0.0, 0.0, 0.0, 1.0)
        }
    }

    fn make_grey_scale(tex: &mut PngResultPersistent) {
        let pixel_step = 4;

        // make the texture gray scale
        for i in 0..tex.width as usize * tex.height as usize {
            let v = ((tex.data[i * pixel_step] as u32
                + tex.data[i * pixel_step + 1] as u32
                + tex.data[i * pixel_step + 2] as u32)
                / 3) as u8;
            tex.data[i * pixel_step] = v;
            tex.data[i * pixel_step + 1] = v;
            tex.data[i * pixel_step + 2] = v;
        }
    }

    fn grey_scale(
        body: &mut PngResultPersistent,
        body_outline: &mut PngResultPersistent,

        left_hand: &mut PngResultPersistent,
        left_hand_outline: &mut PngResultPersistent,

        right_hand: &mut PngResultPersistent,
        right_hand_outline: &mut PngResultPersistent,

        left_foot: &mut PngResultPersistent,
        left_foot_outline: &mut PngResultPersistent,

        right_foot: &mut PngResultPersistent,
        right_foot_outline: &mut PngResultPersistent,

        left_eyes: &mut [PngResultPersistent; TeeSkinEye::Count as usize],

        right_eyes: &mut [PngResultPersistent; TeeSkinEye::Count as usize],
    ) {
        let pixel_step = 4;
        // create grey scales
        Self::make_grey_scale(body);
        Self::make_grey_scale(body_outline);

        Self::make_grey_scale(left_hand);
        Self::make_grey_scale(left_hand_outline);

        Self::make_grey_scale(right_hand);
        Self::make_grey_scale(right_hand_outline);

        Self::make_grey_scale(left_foot);
        Self::make_grey_scale(left_foot_outline);

        Self::make_grey_scale(right_foot);
        Self::make_grey_scale(right_foot_outline);

        left_eyes.iter_mut().for_each(|tex| {
            Self::make_grey_scale(tex);
        });

        right_eyes.iter_mut().for_each(|tex| {
            Self::make_grey_scale(tex);
        });

        let mut freq: [i32; 256] = [0; 256];
        let mut org_weight: i32 = 0;
        let new_weight: i32 = 192;

        // find most common frequence
        for y in 0..body.height as usize {
            for x in 0..body.width as usize {
                if body.data[y + x * pixel_step + 3] > 128 {
                    freq[body.data[y + x * pixel_step] as usize] += 1;
                }
            }
        }

        for i in 1..256 {
            if freq[org_weight as usize] < freq[i as usize] {
                org_weight = i;
            }
        }

        // reorder
        let inv_org_weight = 255 - org_weight;
        let inv_new_weight = 255 - new_weight;
        for y in 0..body.height as usize {
            for x in 0..body.width as usize {
                let mut v = body.data[y + x * pixel_step] as i32;
                if v <= org_weight && org_weight == 0 {
                    v = 0;
                } else if v <= org_weight {
                    v = ((v as f32 / org_weight as f32) * new_weight as f32) as i32;
                } else if inv_org_weight == 0 {
                    v = new_weight;
                } else {
                    v = (((v - org_weight) as f32 / inv_org_weight as f32) * inv_new_weight as f32
                        + new_weight as f32) as i32;
                }
                body.data[y + x * pixel_step] = v as u8;
                body.data[y + x * pixel_step + 1] = v as u8;
                body.data[y + x * pixel_step + 2] = v as u8;
            }
        }
    }

    async fn load_skin(&mut self, fs: &FileSystem, skin_name: &str) -> anyhow::Result<()> {
        self.textures.load_skin(fs, skin_name).await?;

        self.grey_scaled_textures = self.textures.clone();
        Self::grey_scale(
            &mut self.grey_scaled_textures.body,
            &mut self.grey_scaled_textures.body_outline,
            &mut self.grey_scaled_textures.left_hand,
            &mut self.grey_scaled_textures.left_hand_outline,
            &mut self.grey_scaled_textures.right_hand,
            &mut self.grey_scaled_textures.right_hand_outline,
            &mut self.grey_scaled_textures.left_foot,
            &mut self.grey_scaled_textures.left_foot_outline,
            &mut self.grey_scaled_textures.right_foot,
            &mut self.grey_scaled_textures.right_foot_outline,
            &mut self.grey_scaled_textures.left_eyes,
            &mut self.grey_scaled_textures.right_eyes,
        );

        self.blood_color = Self::get_blood_color(
            &self.textures.body.data,
            self.textures.body.width as usize,
            self.textures.body.height as usize,
        );

        self.metrics.body.from_texture(
            &self.textures.body.data,
            self.textures.body.width,
            0,
            0,
            self.textures.body.width,
            self.textures.body.height,
        );
        self.metrics.body.from_texture(
            &self.textures.body_outline.data,
            self.textures.body_outline.width,
            0,
            0,
            self.textures.body_outline.width,
            self.textures.body_outline.height,
        );

        self.metrics.feet.from_texture(
            &self.textures.left_foot.data,
            self.textures.left_foot.width,
            0,
            0,
            self.textures.left_foot.width,
            self.textures.left_foot.height,
        );
        self.metrics.feet.from_texture(
            &self.textures.left_foot_outline.data,
            self.textures.left_foot_outline.width,
            0,
            0,
            self.textures.left_foot_outline.width,
            self.textures.left_foot_outline.height,
        );

        Ok(())
    }

    fn load_file_into_texture(
        graphics: &mut Graphics,
        img: PngResultPersistent,
        name: &str,
    ) -> TextureIndex {
        let texture_id = graphics
            .texture_handle
            .load_texture_slow(
                img.width as usize,
                img.height as usize,
                ImageFormat::Rgba as i32,
                img.data,
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
    async fn load(
        &mut self,
        item_name: &str,
        fs: &Arc<FileSystem>,
        _runtime_thread_pool: &Arc<rayon::ThreadPool>,
    ) -> anyhow::Result<()> {
        self.load_skin(fs, item_name).await?;
        self.skin_name = item_name.to_string();
        Ok(())
    }

    fn convert(self, graphics: &mut Graphics) -> Skin {
        Skin {
            textures: self.textures.to_skin(&self.skin_name, graphics),
            grey_scaled_textures: self.grey_scaled_textures.to_skin(&self.skin_name, graphics),
            metrics: self.metrics,
        }
    }
}

pub type SkinContainer = Container<Skin, LoadSkin>;
