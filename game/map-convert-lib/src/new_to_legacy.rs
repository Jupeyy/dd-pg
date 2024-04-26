use anyhow::anyhow;
use base::{benchmark::Benchmark, hash::fmt_hash};
use base_io::io::IOFileSys;
use map::map::Map;
use shared_base::datafile::CDatafileWrapper;
use std::{path::Path, sync::Arc};

// the map is prepared to be written to disk. the map format is not used in the code base
#[derive(Debug)]
pub struct NewMapToLegacyOutput {
    pub map: Vec<u8>,
}

/// this function will only be supported as long as the map format is equally convertable to the old format
pub fn new_to_legacy(
    path: &Path,
    io: &IOFileSys,
    thread_pool: &Arc<rayon::ThreadPool>,
) -> anyhow::Result<NewMapToLegacyOutput> {
    let fs = io.fs.clone();
    let tp = thread_pool.clone();
    let map_name2 = path.to_path_buf();
    let map_new = io.io_batcher.spawn(async move {
        let path = map_name2.as_ref();
        let map = fs
            .open_file(path)
            .await
            .map_err(|err| anyhow!("loading map file failed: {err}"))?;
        let map =
            Map::read(&map, &tp).map_err(|err| anyhow!("loading map from file failed: {err}"))?;

        let mut images: Vec<Vec<u8>> = Default::default();
        for image in &map.resources.images {
            let img_file = fs
                .open_file(
                    format!(
                        "map/resources/images/{}_{}.{}",
                        image.name,
                        fmt_hash(&image.blake3_hash),
                        image.ty
                    )
                    .as_ref(),
                )
                .await
                .map_err(|err| anyhow!("loading images failed: {err}"))?;
            images.push(img_file);
        }

        let mut image_arrays: Vec<Vec<u8>> = Default::default();
        for image_array in &map.resources.image_arrays {
            let img_file = fs
                .open_file(
                    format!(
                        "map/resources/images/{}_{}.{}",
                        image_array.name,
                        fmt_hash(&image_array.blake3_hash),
                        image_array.ty
                    )
                    .as_ref(),
                )
                .await
                .map_err(|err| anyhow!("loading images failed: {err}"))?;
            image_arrays.push(img_file);
        }

        let mut sounds: Vec<Vec<u8>> = Default::default();
        for sound in &map.resources.sounds {
            let img_file = fs
                .open_file(
                    format!(
                        "map/resources/sounds/{}_{}.{}",
                        sound.name,
                        fmt_hash(&sound.blake3_hash),
                        sound.ty
                    )
                    .as_ref(),
                )
                .await
                .map_err(|err| anyhow!("loading sound failed: {err}"))?;
            sounds.push(img_file);
        }

        Ok((map, images, image_arrays, sounds))
    });
    let (map, images, image_arrays, sounds) = map_new
        .get_storage()
        .map_err(|err| anyhow!("loading map failed: {err}"))?;

    let benchmark = Benchmark::new(true);
    let map_legacy = CDatafileWrapper::from_map(map, &images, &image_arrays, &sounds);
    benchmark.bench("converting to legacy");
    Ok(NewMapToLegacyOutput { map: map_legacy })
}
