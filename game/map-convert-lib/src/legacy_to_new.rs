use std::{
    collections::HashMap,
    io::Cursor,
    num::{NonZeroU32, NonZeroU8},
    path::Path,
    sync::Arc,
};

use anyhow::anyhow;
use base::{
    benchmark::Benchmark,
    hash::{generate_hash_for, Hash},
    reduced_ascii_str::ReducedAsciiString,
};
use base_io::io::IoFileSys;
use image::png::{load_png_image, save_png_image_ex};
use map::map::Map;
use oxipng::optimize_from_memory;
use rayon::iter::{IntoParallelIterator, IntoParallelRefIterator, ParallelIterator};
use shared_base::{
    datafile::{
        CDatafileWrapper, MapFileImageReadOptions, MapFileLayersReadOptions, MapFileOpenOptions,
        MapFileSoundReadOptions,
    },
    mapdef_06::MapSound,
};
use vorbis_rs::VorbisEncoderBuilder;

#[derive(Debug)]
pub struct LegacyMapToNewRes {
    pub buf: Vec<u8>,
    pub ty: String,
    pub name: String,
    pub blake3_hash: Hash,
}

#[derive(Debug)]
pub struct LegacyMapToNewResources {
    pub images: Vec<LegacyMapToNewRes>,
    pub sounds: Vec<LegacyMapToNewRes>,
}

#[derive(Debug)]
pub struct LegacyMapToNewOutput {
    pub map: Map,
    pub resources: LegacyMapToNewResources,
}

pub fn legacy_to_new(
    path: &Path,
    io: &IoFileSys,
    thread_pool: &Arc<rayon::ThreadPool>,
    optimize: bool,
) -> anyhow::Result<LegacyMapToNewOutput> {
    let fs = io.fs.clone();
    let map_name = path.to_path_buf();
    let map_file = io
        .io_batcher
        .spawn(async move {
            let path = map_name.as_ref();
            let map = fs.read_file(path).await?;
            Ok(map)
        })
        .get_storage()?;

    legacy_to_new_from_buf(
        map_file,
        path.file_stem()
            .ok_or(anyhow!("wrong file name"))?
            .to_str()
            .ok_or(anyhow!("file name not utf8"))?,
        io,
        thread_pool,
        optimize,
    )
}

pub fn legacy_to_new_from_buf(
    map_file: Vec<u8>,
    name: &str,
    io: &IoFileSys,
    thread_pool: &Arc<rayon::ThreadPool>,
    optimize: bool,
) -> anyhow::Result<LegacyMapToNewOutput> {
    let tp = thread_pool.clone();
    let name = name.to_string();
    let map_legacy = io.io_batcher.spawn(async move {
        let mut file_wrap = CDatafileWrapper::new();
        let load_options = MapFileOpenOptions::default();
        let res = file_wrap.open(&map_file, &name, tp.as_ref(), &load_options);
        match res {
            Ok(data_start) => {
                CDatafileWrapper::read_map_layers(
                    &file_wrap.data_file,
                    &mut file_wrap.layers,
                    data_start,
                    &MapFileLayersReadOptions::default(),
                );

                let imgs = CDatafileWrapper::read_image_data(
                    &file_wrap.data_file,
                    &file_wrap.images,
                    data_start,
                    &MapFileImageReadOptions {
                        do_benchmark: false,
                    },
                );
                for (i, img) in imgs.into_iter().enumerate() {
                    if let Some((_, _, img)) = img {
                        file_wrap.images[i].internal_img = Some(img);
                    }
                }

                let snds = CDatafileWrapper::read_sound_data(
                    &file_wrap.data_file,
                    &file_wrap.sounds,
                    data_start,
                    &MapFileSoundReadOptions {
                        do_benchmark: false,
                    },
                );
                for (i, snd) in snds.into_iter().enumerate() {
                    if let Some((_, snd)) = snd {
                        file_wrap.sounds[i].data = Some(snd);
                    }
                }
            }
            Err(err) => {
                return Err(anyhow!("map not loaded {err}"));
            }
        }
        file_wrap.init_layers(&tp);

        Ok(file_wrap)
    });
    let map_legacy = map_legacy.get_storage()?;

    let fs = io.fs.clone();
    let read_files = map_legacy.read_files.clone();
    let images = io
        .io_batcher
        .spawn(async move {
            let mut res: Vec<Vec<u8>> = Default::default();
            for read_file_path in read_files.keys() {
                let read_file_path = read_file_path.to_string();
                let file = fs.read_file(read_file_path.as_ref()).await?;
                res.push(file)
            }

            Ok(res)
        })
        .get_storage()?;

    let benchmark = Benchmark::new(true);
    let img_new = thread_pool.install(|| {
        map_legacy
            .images
            .par_iter()
            .map(|i| {
                anyhow::Ok((
                    ReducedAsciiString::from_str_autoconvert(&i.img_name),
                    if let Some((index, _)) = map_legacy
                        .read_files
                        .keys()
                        .enumerate()
                        .find(|(_, name)| **name == format!("legacy/mapres/{}.png", i.img_name))
                    {
                        let mut img_buff: Vec<u8> = Default::default();
                        let img =
                            load_png_image(&images[index], |width, height, color_chanel_count| {
                                img_buff.resize(
                                    width * height * color_chanel_count,
                                    Default::default(),
                                );
                                &mut img_buff
                            })?;
                        save_png_image_ex(img.data, img.width, img.height, true)?
                    } else {
                        save_png_image_ex(
                            i.internal_img
                                .as_ref()
                                .ok_or(anyhow!("internal/embedded image was missing"))?,
                            i.item_data.width as u32,
                            i.item_data.height as u32,
                            true,
                        )?
                    },
                ))
            })
            .collect::<anyhow::Result<HashMap<ReducedAsciiString, Vec<u8>>>>()
    })?;
    let images_new = if optimize {
        thread_pool.install(|| {
            img_new
                .into_par_iter()
                .map(|(name, i)| {
                    anyhow::Ok((name, optimize_from_memory(&i, &oxipng::Options::default())?))
                })
                .collect::<anyhow::Result<HashMap<ReducedAsciiString, Vec<u8>>>>()
        })?
    } else {
        img_new
    };

    let sounds: HashMap<ReducedAsciiString, MapSound> = map_legacy
        .sounds
        .iter()
        .map(|sound| {
            (
                ReducedAsciiString::from_str_autoconvert(&sound.name),
                sound.clone(),
            )
        })
        .collect();

    benchmark.bench("encoding images to png");
    let mut map = map_legacy.into_map(&images)?;
    benchmark.bench("converting map");

    let images: HashMap<Hash, LegacyMapToNewRes> = map
        .resources
        .images
        .iter_mut()
        .chain(map.resources.image_arrays.iter_mut())
        .map(|res| {
            let res_file = images_new.get(&res.name).unwrap();
            res.blake3_hash = generate_hash_for(res_file);
            (
                res.blake3_hash,
                LegacyMapToNewRes {
                    buf: res_file.clone(),
                    ty: "png".to_string(),
                    name: res.name.to_string(),
                    blake3_hash: res.blake3_hash,
                },
            )
        })
        .collect();

    let sounds: HashMap<Hash, LegacyMapToNewRes> = map
        .resources
        .sounds
        .iter_mut()
        .map(|res| {
            let res_file = sounds.get(&res.name).unwrap().data.clone().unwrap();

            // transcode from opus to vorbis
            let (raw, header) = ogg_opus::decode::<_, 48000>(Cursor::new(&res_file)).unwrap();
            let mut transcoded_ogg = vec![];
            let mut encoder = VorbisEncoderBuilder::new_with_serial(
                NonZeroU32::new(48000).unwrap(),
                NonZeroU8::new(2).unwrap(),
                &mut transcoded_ogg,
                0,
            )
            .build()
            .unwrap();

            let (channel1, channel2): (Vec<_>, Vec<_>) = raw
                .chunks_exact(header.channels as usize)
                .map(|freq| {
                    if freq.len() == 1 {
                        (
                            (freq[0] as f64 / i16::MAX as f64) as f32,
                            (freq[0] as f64 / i16::MAX as f64) as f32,
                        )
                    } else {
                        (
                            (freq[0] as f64 / i16::MAX as f64) as f32,
                            (freq[1] as f64 / i16::MAX as f64) as f32,
                        )
                    }
                })
                .unzip();
            encoder.encode_audio_block([channel1, channel2]).unwrap();
            encoder.finish().unwrap();

            res.blake3_hash = generate_hash_for(&transcoded_ogg);
            (
                res.blake3_hash,
                LegacyMapToNewRes {
                    buf: transcoded_ogg,
                    ty: "ogg".to_string(),
                    name: res.name.to_string(),
                    blake3_hash: res.blake3_hash,
                },
            )
        })
        .collect();

    Ok(LegacyMapToNewOutput {
        resources: LegacyMapToNewResources {
            images: images.into_values().collect(),
            sounds: sounds.into_values().collect(),
        },
        map,
    })
}
