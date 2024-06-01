use std::{future::Future, path::Path, pin::Pin, sync::Arc};

use arrayvec::ArrayVec;
use async_trait::async_trait;

use base_io_traits::fs_traits::FileSystemInterface;
use fixed::{types::extra::U32, FixedI64};
use game_interface::types::render::character::{TeeEye, TEE_EYE_COUNT};
use graphics::{
    graphics_mt::GraphicsMultiThreaded,
    handles::texture::texture::{GraphicsTextureHandle, TextureContainer},
};
use graphics_types::{
    commands::{TexFlags, TexFormat},
    rendering::ColorRGBA,
    types::{GraphicsMemoryAllocationType, ImageFormat},
};
use image::png::PngResultPersistent;
use math::math::vector::vec3;
use sound::{
    sound_handle::SoundObjectHandle, sound_mt::SoundMultiThreaded,
    sound_mt_types::SoundBackendMemory, sound_object::SoundObject,
};

use crate::container::load_sound_file_part_and_upload;

use super::container::{load_file_part_as_png, Container, ContainerItemLoadData, ContainerLoad};

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

pub struct SkinSounds {
    pub ground_jump: [SoundObject; 8],
    pub air_jump: [SoundObject; 3],
    pub spawn: [SoundObject; 7],
    pub death: [SoundObject; 4],
    pub pain_short: [SoundObject; 12],
    pub pain_long: [SoundObject; 2],
    pub hit_weak: [SoundObject; 3],
    pub hit_strong: [SoundObject; 2],
}

pub struct SkinTextures {
    pub body: TextureContainer,
    pub body_outline: TextureContainer,

    pub marking: TextureContainer,
    pub marking_outline: TextureContainer,

    pub decoration: TextureContainer,
    pub decoration_outline: TextureContainer,

    pub left_hand: TextureContainer,
    pub left_hand_outline: TextureContainer,

    pub right_hand: TextureContainer,
    pub right_hand_outline: TextureContainer,

    pub left_foot: TextureContainer,
    pub left_foot_outline: TextureContainer,

    pub right_foot: TextureContainer,
    pub right_foot_outline: TextureContainer,

    pub left_eyes: [TextureContainer; TEE_EYE_COUNT],
    pub right_eyes: [TextureContainer; TEE_EYE_COUNT],
}

pub struct Skin {
    pub textures: SkinTextures,
    pub grey_scaled_textures: SkinTextures,
    pub metrics: SkinMetrics,

    pub blood_color: ColorRGBA,

    pub sounds: SkinSounds,
}

#[derive(Debug)]
pub struct LoadSkinSounds {
    pub ground_jump: [SoundBackendMemory; 8],
    pub air_jump: [SoundBackendMemory; 3],
    pub spawn: [SoundBackendMemory; 7],
    pub death: [SoundBackendMemory; 4],
    pub pain_short: [SoundBackendMemory; 12],
    pub pain_long: [SoundBackendMemory; 2],
    pub hit_weak: [SoundBackendMemory; 3],
    pub hit_strong: [SoundBackendMemory; 2],
}

impl LoadSkinSounds {
    fn load_into_sound_object(self, sound_object_handle: &SoundObjectHandle) -> SkinSounds {
        SkinSounds {
            ground_jump: {
                let mut jumps: Vec<_> = Vec::new();
                for jump in self.ground_jump.into_iter() {
                    jumps.push(sound_object_handle.create(jump));
                }
                jumps.try_into().unwrap()
            },
            air_jump: {
                let mut jumps: Vec<_> = Vec::new();
                for jump in self.air_jump.into_iter() {
                    jumps.push(sound_object_handle.create(jump));
                }
                jumps.try_into().unwrap()
            },
            spawn: {
                let mut sounds: Vec<_> = Vec::new();
                for snd in self.spawn.into_iter() {
                    sounds.push(sound_object_handle.create(snd));
                }
                sounds.try_into().unwrap()
            },
            death: {
                let mut sounds: Vec<_> = Vec::new();
                for snd in self.death.into_iter() {
                    sounds.push(sound_object_handle.create(snd));
                }
                sounds.try_into().unwrap()
            },
            pain_short: self
                .pain_short
                .into_iter()
                .map(|snd| sound_object_handle.create(snd))
                .collect::<Vec<_>>()
                .try_into()
                .unwrap(),
            pain_long: self
                .pain_long
                .into_iter()
                .map(|snd| sound_object_handle.create(snd))
                .collect::<Vec<_>>()
                .try_into()
                .unwrap(),
            hit_strong: self
                .hit_strong
                .into_iter()
                .map(|snd| sound_object_handle.create(snd))
                .collect::<Vec<_>>()
                .try_into()
                .unwrap(),
            hit_weak: self
                .hit_weak
                .into_iter()
                .map(|snd| sound_object_handle.create(snd))
                .collect::<Vec<_>>()
                .try_into()
                .unwrap(),
        }
    }
}

#[derive(Default, Clone)]
pub struct LoadSkinTexturesData {
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

    left_eyes: [PngResultPersistent; TEE_EYE_COUNT],
    right_eyes: [PngResultPersistent; TEE_EYE_COUNT],
}

impl LoadSkinTexturesData {
    pub(crate) async fn load_skin(
        fs: &dyn FileSystemInterface,
        skin_name: &str,
        skin_extra_path: Option<&str>,
        collection_path: &Path,
    ) -> anyhow::Result<Self> {
        let skin_path = collection_path;

        let load_eyes = |eye_name: &'static str| -> Pin<
            Box<dyn Future<Output = anyhow::Result<[PngResultPersistent; TEE_EYE_COUNT]>> + Send>,
        > {
            Box::pin(async {
                let mut eyes: [PngResultPersistent; TEE_EYE_COUNT] = Default::default();
                let extra_paths = [skin_extra_path.as_slice(), &[eye_name]].concat();
                eyes[TeeEye::Angry as usize] = load_file_part_as_png(
                    fs,
                    &skin_path,
                    skin_name,
                    extra_paths.as_slice(),
                    "angry",
                )
                .await?;
                eyes[TeeEye::Dead as usize] = load_file_part_as_png(
                    fs,
                    &skin_path,
                    skin_name,
                    extra_paths.as_slice(),
                    "dead",
                )
                .await?;
                eyes[TeeEye::Happy as usize] = load_file_part_as_png(
                    fs,
                    &skin_path,
                    skin_name,
                    extra_paths.as_slice(),
                    "happy",
                )
                .await?;
                eyes[TeeEye::Normal as usize] = load_file_part_as_png(
                    fs,
                    &skin_path,
                    skin_name,
                    extra_paths.as_slice(),
                    "normal",
                )
                .await?;
                eyes[TeeEye::Blink as usize] = load_file_part_as_png(
                    fs,
                    &skin_path,
                    skin_name,
                    extra_paths.as_slice(),
                    "normal",
                )
                .await?; // TODO: wrong
                eyes[TeeEye::Pain as usize] = load_file_part_as_png(
                    fs,
                    &skin_path,
                    skin_name,
                    extra_paths.as_slice(),
                    "pain",
                )
                .await?;
                eyes[TeeEye::Surprised as usize] = load_file_part_as_png(
                    fs,
                    &skin_path,
                    skin_name,
                    extra_paths.as_slice(),
                    "surprised",
                )
                .await?;
                Ok(eyes)
            })
        };

        Ok(Self {
            // body file
            body: load_file_part_as_png(
                fs,
                &skin_path,
                skin_name,
                skin_extra_path.as_slice(),
                "body",
            )
            .await?,
            body_outline: load_file_part_as_png(
                fs,
                &skin_path,
                skin_name,
                skin_extra_path.as_slice(),
                "body_outline",
            )
            .await?,

            // foot_left file
            left_foot: load_file_part_as_png(
                fs,
                &skin_path,
                skin_name,
                skin_extra_path.as_slice(),
                "foot_left",
            )
            .await?,
            left_foot_outline: load_file_part_as_png(
                fs,
                &skin_path,
                skin_name,
                skin_extra_path.as_slice(),
                "foot_left_outline",
            )
            .await?,

            // foot_right file
            right_foot: load_file_part_as_png(
                fs,
                &skin_path,
                skin_name,
                skin_extra_path.as_slice(),
                "foot_right",
            )
            .await?,
            right_foot_outline: load_file_part_as_png(
                fs,
                &skin_path,
                skin_name,
                skin_extra_path.as_slice(),
                "foot_right_outline",
            )
            .await?,

            // hand_left file
            left_hand: load_file_part_as_png(
                fs,
                &skin_path,
                skin_name,
                skin_extra_path.as_slice(),
                "hand_left",
            )
            .await?,
            left_hand_outline: load_file_part_as_png(
                fs,
                &skin_path,
                skin_name,
                skin_extra_path.as_slice(),
                "hand_left_outline",
            )
            .await?,

            // hand_right file
            right_hand: load_file_part_as_png(
                fs,
                &skin_path,
                skin_name,
                skin_extra_path.as_slice(),
                "hand_right",
            )
            .await?,
            right_hand_outline: load_file_part_as_png(
                fs,
                &skin_path,
                skin_name,
                skin_extra_path.as_slice(),
                "hand_right_outline",
            )
            .await?,

            // eyes file
            left_eyes: load_eyes("eyes_left").await?,
            right_eyes: load_eyes("eyes_right").await?,

            // decoration file
            decoration: load_file_part_as_png(
                fs,
                &skin_path,
                skin_name,
                skin_extra_path.as_slice(),
                "decoration",
            )
            .await?,
            decoration_outline: load_file_part_as_png(
                fs,
                &skin_path,
                skin_name,
                skin_extra_path.as_slice(),
                "decoration",
            )
            .await?,

            // marking file
            marking: load_file_part_as_png(
                fs,
                &skin_path,
                skin_name,
                skin_extra_path.as_slice(),
                "marking",
            )
            .await?,
            marking_outline: load_file_part_as_png(
                fs,
                &skin_path,
                skin_name,
                skin_extra_path.as_slice(),
                "marking",
            )
            .await?,
        })
    }

    fn load_single(
        graphics_mt: &GraphicsMultiThreaded,
        img: PngResultPersistent,
    ) -> ContainerItemLoadData {
        let mut img_mem = graphics_mt.mem_alloc(GraphicsMemoryAllocationType::Texture {
            width: img.width as usize,
            height: img.height as usize,
            depth: 1,
            is_3d_tex: false,
            flags: TexFlags::empty(),
        });
        img_mem.as_mut_slice().copy_from_slice(&img.data);
        if graphics_mt.try_flush_mem(&mut img_mem, true).is_err() {
            // TODO: ignore?
        }
        ContainerItemLoadData {
            width: img.width,
            height: img.height,
            depth: 1,
            data: img_mem,
        }
    }

    fn load_into_texture(self, graphics_mt: &GraphicsMultiThreaded) -> LoadSkinTextures {
        LoadSkinTextures {
            body: Self::load_single(graphics_mt, self.body),
            body_outline: Self::load_single(graphics_mt, self.body_outline),
            marking: Self::load_single(graphics_mt, self.marking),
            marking_outline: Self::load_single(graphics_mt, self.marking_outline),
            decoration: Self::load_single(graphics_mt, self.decoration),
            decoration_outline: Self::load_single(graphics_mt, self.decoration_outline),
            left_hand: Self::load_single(graphics_mt, self.left_hand),
            left_hand_outline: Self::load_single(graphics_mt, self.left_hand_outline),
            right_hand: Self::load_single(graphics_mt, self.right_hand),
            right_hand_outline: Self::load_single(graphics_mt, self.right_hand_outline),
            left_foot: Self::load_single(graphics_mt, self.left_foot),
            left_foot_outline: Self::load_single(graphics_mt, self.left_foot_outline),
            right_foot: Self::load_single(graphics_mt, self.right_foot),
            right_foot_outline: Self::load_single(graphics_mt, self.right_foot_outline),
            left_eyes: self
                .left_eyes
                .into_iter()
                .map(|eye| Self::load_single(graphics_mt, eye))
                .collect::<ArrayVec<_, { TEE_EYE_COUNT }>>()
                .into_inner()
                .unwrap(),
            right_eyes: self
                .right_eyes
                .into_iter()
                .map(|eye| Self::load_single(graphics_mt, eye))
                .collect::<ArrayVec<_, { TEE_EYE_COUNT }>>()
                .into_inner()
                .unwrap(),
        }
    }
}

#[derive(Debug)]
pub struct LoadSkinTextures {
    body: ContainerItemLoadData,
    body_outline: ContainerItemLoadData,

    marking: ContainerItemLoadData,
    marking_outline: ContainerItemLoadData,

    decoration: ContainerItemLoadData,
    decoration_outline: ContainerItemLoadData,

    left_hand: ContainerItemLoadData,
    left_hand_outline: ContainerItemLoadData,

    right_hand: ContainerItemLoadData,
    right_hand_outline: ContainerItemLoadData,

    left_foot: ContainerItemLoadData,
    left_foot_outline: ContainerItemLoadData,

    right_foot: ContainerItemLoadData,
    right_foot_outline: ContainerItemLoadData,

    left_eyes: [ContainerItemLoadData; TEE_EYE_COUNT],
    right_eyes: [ContainerItemLoadData; TEE_EYE_COUNT],
}

impl LoadSkinTextures {
    fn load_skin_into_texture(
        self,
        skin_name: &str,
        texture_handle: &GraphicsTextureHandle,
    ) -> SkinTextures {
        SkinTextures {
            body: LoadSkin::load_file_into_texture(texture_handle, self.body, skin_name),
            body_outline: LoadSkin::load_file_into_texture(
                texture_handle,
                self.body_outline,
                skin_name,
            ),
            marking: LoadSkin::load_file_into_texture(texture_handle, self.marking, skin_name),
            marking_outline: LoadSkin::load_file_into_texture(
                texture_handle,
                self.marking_outline,
                skin_name,
            ),
            decoration: LoadSkin::load_file_into_texture(
                texture_handle,
                self.decoration,
                skin_name,
            ),
            decoration_outline: LoadSkin::load_file_into_texture(
                texture_handle,
                self.decoration_outline,
                skin_name,
            ),
            left_hand: LoadSkin::load_file_into_texture(texture_handle, self.left_hand, skin_name),
            left_hand_outline: LoadSkin::load_file_into_texture(
                texture_handle,
                self.left_hand_outline,
                skin_name,
            ),
            right_hand: LoadSkin::load_file_into_texture(
                texture_handle,
                self.right_hand,
                skin_name,
            ),
            right_hand_outline: LoadSkin::load_file_into_texture(
                texture_handle,
                self.right_hand_outline,
                skin_name,
            ),
            left_foot: LoadSkin::load_file_into_texture(texture_handle, self.left_foot, skin_name),
            left_foot_outline: LoadSkin::load_file_into_texture(
                texture_handle,
                self.left_foot_outline,
                skin_name,
            ),
            right_foot: LoadSkin::load_file_into_texture(
                texture_handle,
                self.right_foot,
                skin_name,
            ),
            right_foot_outline: LoadSkin::load_file_into_texture(
                texture_handle,
                self.right_foot_outline,
                skin_name,
            ),
            left_eyes: self
                .left_eyes
                .into_iter()
                .map(|eye| LoadSkin::load_file_into_texture(texture_handle, eye, skin_name))
                .collect::<ArrayVec<_, { TEE_EYE_COUNT }>>()
                .into_inner()
                .unwrap(),
            right_eyes: self
                .right_eyes
                .into_iter()
                .map(|eye| LoadSkin::load_file_into_texture(texture_handle, eye, skin_name))
                .collect::<ArrayVec<_, { TEE_EYE_COUNT }>>()
                .into_inner()
                .unwrap(),
        }
    }
}

#[derive(Debug)]
pub struct LoadSkin {
    textures: LoadSkinTextures,
    grey_scaled_textures: LoadSkinTextures,

    blood_color: ColorRGBA,

    metrics: SkinMetrics,

    sound: LoadSkinSounds,

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
        body_and_outline: (&mut PngResultPersistent, &mut PngResultPersistent),

        left_hand_and_outline: (&mut PngResultPersistent, &mut PngResultPersistent),

        right_hand_and_outline: (&mut PngResultPersistent, &mut PngResultPersistent),

        left_foot_and_outline: (&mut PngResultPersistent, &mut PngResultPersistent),

        right_foot_and_outline: (&mut PngResultPersistent, &mut PngResultPersistent),

        left_eyes: &mut [PngResultPersistent; TEE_EYE_COUNT],

        right_eyes: &mut [PngResultPersistent; TEE_EYE_COUNT],
    ) {
        let pixel_step = 4;
        // create grey scales
        let (body, body_outline) = body_and_outline;
        Self::make_grey_scale(body);
        Self::make_grey_scale(body_outline);

        let (left_hand, left_hand_outline) = left_hand_and_outline;
        Self::make_grey_scale(left_hand);
        Self::make_grey_scale(left_hand_outline);

        let (right_hand, right_hand_outline) = right_hand_and_outline;
        Self::make_grey_scale(right_hand);
        Self::make_grey_scale(right_hand_outline);

        let (left_foot, left_foot_outline) = left_foot_and_outline;
        Self::make_grey_scale(left_foot);
        Self::make_grey_scale(left_foot_outline);

        let (right_foot, right_foot_outline) = right_foot_and_outline;
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

        let body_pitch = body.width as usize * pixel_step;

        // find most common frequence
        for y in 0..body.height as usize {
            for x in 0..body.width as usize {
                if body.data[y * body_pitch + x * pixel_step + 3] > 128 {
                    freq[body.data[y * body_pitch + x * pixel_step] as usize] += 1;
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
                let mut v = body.data[y * body_pitch + x * pixel_step] as i32;
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
                body.data[y * body_pitch + x * pixel_step] = v as u8;
                body.data[y * body_pitch + x * pixel_step + 1] = v as u8;
                body.data[y * body_pitch + x * pixel_step + 2] = v as u8;
            }
        }
    }

    pub(crate) async fn load_skin(
        graphics_mt: &GraphicsMultiThreaded,
        sound_mt: &SoundMultiThreaded,
        fs: &dyn FileSystemInterface,
        skin_name: &str,
        skin_extra_path: Option<&str>,
        collection_path: &Path,
    ) -> anyhow::Result<Self> {
        let textures_data =
            LoadSkinTexturesData::load_skin(fs, skin_name, skin_extra_path, collection_path)
                .await?;

        let mut grey_scaled_textures_data = textures_data.clone();
        Self::grey_scale(
            (
                &mut grey_scaled_textures_data.body,
                &mut grey_scaled_textures_data.body_outline,
            ),
            (
                &mut grey_scaled_textures_data.left_hand,
                &mut grey_scaled_textures_data.left_hand_outline,
            ),
            (
                &mut grey_scaled_textures_data.right_hand,
                &mut grey_scaled_textures_data.right_hand_outline,
            ),
            (
                &mut grey_scaled_textures_data.left_foot,
                &mut grey_scaled_textures_data.left_foot_outline,
            ),
            (
                &mut grey_scaled_textures_data.right_foot,
                &mut grey_scaled_textures_data.right_foot_outline,
            ),
            &mut grey_scaled_textures_data.left_eyes,
            &mut grey_scaled_textures_data.right_eyes,
        );

        let mut metrics_body = SkinMetricVariable::default();
        metrics_body.from_texture(
            &textures_data.body.data,
            textures_data.body.width,
            0,
            0,
            textures_data.body.width,
            textures_data.body.height,
        );
        metrics_body.from_texture(
            &textures_data.body_outline.data,
            textures_data.body_outline.width,
            0,
            0,
            textures_data.body_outline.width,
            textures_data.body_outline.height,
        );

        let mut metrics_feet = SkinMetricVariable::default();
        metrics_feet.from_texture(
            &textures_data.left_foot.data,
            textures_data.left_foot.width,
            0,
            0,
            textures_data.left_foot.width,
            textures_data.left_foot.height,
        );
        metrics_feet.from_texture(
            &textures_data.left_foot_outline.data,
            textures_data.left_foot_outline.width,
            0,
            0,
            textures_data.left_foot_outline.width,
            textures_data.left_foot_outline.height,
        );

        Ok(Self {
            blood_color: Self::get_blood_color(
                &textures_data.body.data,
                textures_data.body.width as usize,
                textures_data.body.height as usize,
            ),
            metrics: SkinMetrics {
                body: metrics_body,
                feet: metrics_feet,
            },

            textures: textures_data.load_into_texture(graphics_mt),
            grey_scaled_textures: grey_scaled_textures_data.load_into_texture(graphics_mt),

            sound: LoadSkinSounds {
                ground_jump: {
                    let mut jumps: Vec<_> = Vec::new();

                    for i in 0..8 {
                        jumps.push(
                            load_sound_file_part_and_upload(
                                sound_mt,
                                fs,
                                collection_path,
                                skin_name,
                                &[skin_extra_path.as_slice(), &["audio"]].concat(),
                                &format!("ground_jump{}", i + 1),
                            )
                            .await?,
                        )
                    }

                    jumps.try_into().unwrap()
                },
                air_jump: {
                    let mut jumps: Vec<_> = Vec::new();

                    for i in 0..3 {
                        jumps.push(
                            load_sound_file_part_and_upload(
                                sound_mt,
                                fs,
                                collection_path,
                                skin_name,
                                &[skin_extra_path.as_slice(), &["audio"]].concat(),
                                &format!("air_jump{}", i + 1),
                            )
                            .await?,
                        )
                    }

                    jumps.try_into().unwrap()
                },
                spawn: {
                    let mut sounds: Vec<_> = Vec::new();

                    for i in 0..7 {
                        sounds.push(
                            load_sound_file_part_and_upload(
                                sound_mt,
                                fs,
                                collection_path,
                                skin_name,
                                &[skin_extra_path.as_slice(), &["audio"]].concat(),
                                &format!("spawn{}", i + 1),
                            )
                            .await?,
                        )
                    }

                    sounds.try_into().unwrap()
                },
                death: {
                    let mut sounds: Vec<_> = Vec::new();

                    for i in 0..4 {
                        sounds.push(
                            load_sound_file_part_and_upload(
                                sound_mt,
                                fs,
                                collection_path,
                                skin_name,
                                &[skin_extra_path.as_slice(), &["audio"]].concat(),
                                &format!("death{}", i + 1),
                            )
                            .await?,
                        )
                    }

                    sounds.try_into().unwrap()
                },
                pain_short: {
                    let mut sounds: Vec<_> = Vec::new();

                    for i in 0..12 {
                        sounds.push(
                            load_sound_file_part_and_upload(
                                sound_mt,
                                fs,
                                collection_path,
                                skin_name,
                                &[skin_extra_path.as_slice(), &["audio"]].concat(),
                                &format!("pain_short{}", i + 1),
                            )
                            .await?,
                        )
                    }

                    sounds.try_into().unwrap()
                },
                pain_long: {
                    let mut sounds: Vec<_> = Vec::new();

                    for i in 0..2 {
                        sounds.push(
                            load_sound_file_part_and_upload(
                                sound_mt,
                                fs,
                                collection_path,
                                skin_name,
                                &[skin_extra_path.as_slice(), &["audio"]].concat(),
                                &format!("pain_long{}", i + 1),
                            )
                            .await?,
                        )
                    }

                    sounds.try_into().unwrap()
                },
                hit_strong: {
                    let mut sounds: Vec<_> = Vec::new();

                    for i in 0..2 {
                        sounds.push(
                            load_sound_file_part_and_upload(
                                sound_mt,
                                fs,
                                collection_path,
                                skin_name,
                                &[skin_extra_path.as_slice(), &["audio"]].concat(),
                                &format!("hit_strong{}", i + 1),
                            )
                            .await?,
                        )
                    }

                    sounds.try_into().unwrap()
                },
                hit_weak: {
                    let mut sounds: Vec<_> = Vec::new();

                    for i in 0..3 {
                        sounds.push(
                            load_sound_file_part_and_upload(
                                sound_mt,
                                fs,
                                collection_path,
                                skin_name,
                                &[skin_extra_path.as_slice(), &["audio"]].concat(),
                                &format!("hit_weak{}", i + 1),
                            )
                            .await?,
                        )
                    }

                    sounds.try_into().unwrap()
                },
            },

            skin_name: skin_name.to_string(),
        })
    }

    pub(crate) fn load_file_into_texture(
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
}

#[async_trait]
impl ContainerLoad<Skin> for LoadSkin {
    async fn load(
        item_name: &str,
        fs: &Arc<dyn FileSystemInterface>,
        _runtime_thread_pool: &Arc<rayon::ThreadPool>,
        graphics_mt: &GraphicsMultiThreaded,
        sound_mt: &SoundMultiThreaded,
    ) -> anyhow::Result<Self> {
        Self::load_skin(
            graphics_mt,
            sound_mt,
            fs.as_ref(),
            item_name,
            None,
            "skins/".as_ref(),
        )
        .await
    }

    fn convert(
        self,
        texture_handle: &GraphicsTextureHandle,
        sound_object_handle: &SoundObjectHandle,
    ) -> Skin {
        Skin {
            textures: self
                .textures
                .load_skin_into_texture(&self.skin_name, texture_handle),
            grey_scaled_textures: self
                .grey_scaled_textures
                .load_skin_into_texture(&self.skin_name, texture_handle),
            metrics: self.metrics,
            blood_color: self.blood_color,

            sounds: self.sound.load_into_sound_object(sound_object_handle),
        }
    }
}

pub type SkinContainer = Container<Skin, LoadSkin>;
