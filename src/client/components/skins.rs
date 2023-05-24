use std::{collections::HashMap, str::FromStr};

use arrayvec::ArrayString;

use crate::client::{
    component::{
        ComponentComponent, ComponentGameMsg, ComponentLoadIOPipe, ComponentLoadPipe,
        ComponentLoadWhileIOPipe, ComponentLoadable, ComponentRenderable, ComponentUpdatable,
    },
    image::png::load_png_image,
};

use graphics::graphics::{Graphics, GraphicsTextureAllocations};

use graphics_types::{
    command_buffer::{TexFlags, TexFormat},
    rendering::ETextureIndex,
    types::ImageFormat,
};

use base::{filesys::FileSystem, io_batcher::IOBatcherTask};

#[derive(Default, Clone)]
pub struct Skin {
    body: ETextureIndex,
    marking: ETextureIndex,
    decoration: ETextureIndex,
    left_hand: ETextureIndex,
    right_hand: ETextureIndex,
    left_foot: ETextureIndex,
    right_foot: ETextureIndex,
    left_eye: ETextureIndex,
    right_eye: ETextureIndex,
}

#[derive(Clone)]
pub struct LoadSkin {
    body: Vec<u8>,
    marking: Vec<u8>,
    decoration: Vec<u8>,
    left_hand: Vec<u8>,
    right_hand: Vec<u8>,
    left_foot: Vec<u8>,
    right_foot: Vec<u8>,
    left_eye: Vec<u8>,
    right_eye: Vec<u8>,
}

impl LoadSkin {
    pub fn new(file: &Vec<u8>) -> Self {
        Self {
            body: file.clone(),
            marking: file.clone(),
            decoration: file.clone(),
            left_hand: file.clone(),
            right_hand: file.clone(),
            left_foot: file.clone(),
            right_foot: file.clone(),
            left_eye: file.clone(),
            right_eye: file.clone(),
        }
    }
}

/**
 * The skin component initializes all skins in a
 * lazy loaded style.
 * The only exception to this is the default skin
 * which must always be available.
 * For UI it provides a list of skin names, so the UI can load skins on fly
 *
 */
pub struct Skins {
    pub skins: HashMap<String, Skin>,
    pub load_task: Option<IOBatcherTask<HashMap<String, LoadSkin>>>,
}

impl ComponentLoadable for Skins {
    fn load_io(&mut self, io_pipe: &mut ComponentLoadIOPipe) {
        let fs = io_pipe.fs.clone();
        self.load_task = Some(
            io_pipe
                .batcher
                .lock()
                .unwrap()
                .spawn::<HashMap<String, LoadSkin>, _>(async move {
                    let mut storage = HashMap::<String, LoadSkin>::default();
                    let def_skin = Self::load_skin(&fs, &mut storage, "default").await;
                    if let Err(err) = def_skin {
                        return Err(err);
                    }
                    Ok(storage)
                }),
        );
    }

    fn init_while_io(&mut self, _pipe: &mut ComponentLoadWhileIOPipe) {}

    fn init(&mut self, pipe: &mut ComponentLoadPipe) -> Result<(), ArrayString<4096>> {
        let load_skins = self.load_task.as_mut().unwrap().get_storage().unwrap();
        for (load_skin_name, load_skin) in load_skins {
            let mut skin = Skin::default();
            skin.body =
                Self::load_file_into_texture(pipe.graphics, &load_skin.body, &load_skin_name);
            skin.marking =
                Self::load_file_into_texture(pipe.graphics, &load_skin.marking, &load_skin_name);
            skin.decoration =
                Self::load_file_into_texture(pipe.graphics, &load_skin.decoration, &load_skin_name);
            skin.left_hand =
                Self::load_file_into_texture(pipe.graphics, &load_skin.left_hand, &load_skin_name);
            skin.right_hand =
                Self::load_file_into_texture(pipe.graphics, &load_skin.right_hand, &load_skin_name);
            skin.left_foot =
                Self::load_file_into_texture(pipe.graphics, &load_skin.left_foot, &load_skin_name);
            skin.right_foot =
                Self::load_file_into_texture(pipe.graphics, &load_skin.right_foot, &load_skin_name);
            skin.left_eye =
                Self::load_file_into_texture(pipe.graphics, &load_skin.left_eye, &load_skin_name);
            skin.right_eye =
                Self::load_file_into_texture(pipe.graphics, &load_skin.right_eye, &load_skin_name);
            self.skins.insert(load_skin_name, skin);
        }
        Ok(())
    }
}

impl ComponentUpdatable for Skins {}

impl ComponentRenderable for Skins {}

impl ComponentGameMsg for Skins {}

impl ComponentComponent for Skins {}

impl Skins {
    pub fn new() -> Self {
        Self {
            skins: Default::default(),
            load_task: None,
        }
    }

    fn load_file_into_texture(
        graphics: &mut Graphics,
        file: &Vec<u8>,
        name: &str,
    ) -> ETextureIndex {
        let mut img_data = Vec::<u8>::new();
        let part_img = load_png_image(&file, |size| {
            img_data = vec![0; size];
            &mut img_data
        })
        .unwrap();
        let mut texture_id = Default::default();
        graphics.load_texture_slow(
            &mut texture_id,
            part_img.width as usize,
            part_img.height as usize,
            ImageFormat::Rgba as i32,
            part_img.data.to_vec(),
            TexFormat::RGBA as i32,
            TexFlags::empty(),
            name,
        );
        texture_id
    }

    async fn load_skin_part(
        fs: &FileSystem,
        skin_path: &ArrayString<4096>,
        part: &str,
        is_default: bool,
    ) -> Result<Vec<u8>, ArrayString<4096>> {
        let mut skin_full_path = *skin_path;
        skin_full_path.push_str(part);
        skin_full_path.push_str(".png");

        let file = fs.open_file(skin_full_path.as_str()).await;

        if let Err(err) = file {
            if !is_default {
                // try to load default part instead
                let mut skin_path_def = ArrayString::<4096>::from_str("skins/").unwrap();
                skin_path_def.push_str("default");
                skin_path_def.push_str("/");
                skin_path_def.push_str(part);
                skin_path_def.push_str(".png");
                let file_def = fs.open_file(skin_full_path.as_str()).await;
                if let Err(err) = file_def {
                    return Err(ArrayString::from(
                        ("default skin part (".to_string()
                            + &part.to_string()
                            + &") not found: ".to_string()
                            + &err.to_string())
                            .as_str(),
                    )
                    .unwrap());
                } else {
                    return Ok(file_def.unwrap());
                }
            } else {
                return Err(ArrayString::from(
                    ("default skin part (".to_string()
                        + &part.to_string()
                        + &") not found: ".to_string()
                        + &err.to_string())
                        .as_str(),
                )
                .unwrap());
            }
        } else {
            return Ok(file.unwrap());
        }
    }

    pub async fn load_skin(
        fs: &FileSystem,
        skin_map: &mut HashMap<String, LoadSkin>,
        skin_name: &str,
    ) -> Result<(), ArrayString<4096>> {
        let mut skin_path = ArrayString::<4096>::from_str("skins/").unwrap();
        skin_path.push_str(skin_name);
        skin_path.push_str("/");

        let is_default = skin_name == "default";

        // body file
        let body = Self::load_skin_part(fs, &skin_path, "body", is_default).await?;

        // foot_left file
        let _foot_left = Self::load_skin_part(fs, &skin_path, "foot_left", is_default).await?;

        // foot_right file
        let _foot_right = Self::load_skin_part(fs, &skin_path, "foot_right", is_default).await?;

        // hand_left file
        let _hand_left = Self::load_skin_part(fs, &skin_path, "hand_left", is_default).await?;

        // hand_right file
        let _hand_right = Self::load_skin_part(fs, &skin_path, "hand_right", is_default).await?;

        // eye_left file
        let _eye_left = Self::load_skin_part(fs, &skin_path, "eye_left", is_default).await?;

        // eye_right file
        let _eye_right = Self::load_skin_part(fs, &skin_path, "eye_right", is_default).await?;

        // decoration file
        let _decoration = Self::load_skin_part(fs, &skin_path, "decoration", is_default).await?;

        // marking file
        let _marking = Self::load_skin_part(fs, &skin_path, "marking", is_default).await?;

        skin_map.insert(skin_name.to_string(), LoadSkin::new(&body));

        Ok(())
    }
}
