use std::{
    num::NonZeroUsize,
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::anyhow;
use base::{benchmark::Benchmark, hash::fmt_hash, system::System};
use base_fs::filesys::FileSystem;
use base_io::io::IoFileSys;
use clap::Parser;
use map::map::Map;
use map_convert_lib::{legacy_to_new::legacy_to_new, new_to_legacy::new_to_legacy};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// file name of the map to convert. (Legacy maps expect mapres to be in the io path, new maps expect map dir to be there)
    file: String,
    /// output path (directory)
    output: String,
    /// optimize PNGs with oxipng (default: on). This option only has an effect when converting legacy maps to new ones
    #[arg(short, long, default_value_t = true)]
    optimize: bool,
    /// export as json (only works for .twmap maps)
    #[arg(short, long, default_value_t = false)]
    json: bool,
}

fn main() {
    let args = Args::parse();

    let sys = System::new();
    let io = IoFileSys::new(|rt| {
        Arc::new(FileSystem::new(
            rt,
            &sys.log,
            "org",
            "",
            "DDNet",
            "DDNet-Accounts",
        ))
    });

    let thread_pool = Arc::new(
        rayon::ThreadPoolBuilder::new()
            .num_threads(
                std::thread::available_parallelism()
                    .unwrap_or(NonZeroUsize::new(2).unwrap())
                    .get(),
            )
            .build()
            .unwrap(),
    );

    let file_path: &Path = args.file.as_ref();
    let file_path = file_path.to_path_buf();
    // legacy to new
    if file_path.extension().is_some_and(|e| e == "map") {
        let output = legacy_to_new(args.file.as_ref(), &io, &thread_pool, args.optimize).unwrap();

        // write map
        let benchmark = Benchmark::new(true);
        let mut file: Vec<u8> = Default::default();
        output.map.write(&mut file, &thread_pool).unwrap();
        benchmark.bench("serializing & compressing map");
        let fs = io.fs.clone();
        let output_dir = args.output.clone();
        io.io_batcher.spawn_without_lifetime(async move {
            fs.create_dir(output_dir.as_ref()).await?;
            // write map
            let mut map_path = PathBuf::from(&output_dir);
            map_path.push("map/maps/");
            fs.create_dir(&map_path).await?;
            map_path.push(format!(
                "{}.twmap",
                file_path.file_stem().unwrap().to_str().unwrap(),
            ));
            fs.write_file(&map_path, file).await?;

            // write resources
            let mut res_path = PathBuf::from(&output_dir);
            res_path.push("map/resources/images/");
            fs.create_dir(&res_path).await?;
            for image in output.resources.images.into_iter() {
                let mut res_path = res_path.clone();
                res_path.push(format!(
                    "{}_{}.{}",
                    image.name,
                    fmt_hash(&image.blake3_hash),
                    image.ty
                ));
                fs.write_file(&res_path, image.buf.clone()).await?;
            }

            let mut res_path = PathBuf::from(&output_dir);
            res_path.push("map/resources/sounds/");
            fs.create_dir(&res_path).await?;
            for sound in output.resources.sounds.into_iter() {
                let mut res_path = res_path.clone();
                res_path.push(format!(
                    "{}_{}.{}",
                    sound.name,
                    fmt_hash(&sound.blake3_hash),
                    sound.ty
                ));
                fs.write_file(&res_path, sound.buf.clone()).await?;
            }

            Ok(())
        });
    }
    // new to legacy
    else if file_path.extension().is_some_and(|e| e == "twmap") {
        if args.json {
            let fs = io.fs.clone();
            let tp = thread_pool.clone();
            let map = io.io_batcher.spawn(async move {
                let path = args.file.as_ref();
                let map = fs
                    .read_file(path)
                    .await
                    .map_err(|err| anyhow!("loading map file failed: {err}"))?;
                let map = Map::read(&map, &tp)
                    .map_err(|err| anyhow!("loading map from file failed: {err}"))?;

                Ok(map)
            });
            let map = map.get_storage().unwrap();
            let fs = io.fs.clone();
            let output_dir = args.output.clone();
            io.io_batcher.spawn_without_lifetime(async move {
                fs.create_dir(output_dir.as_ref()).await?;
                // write map
                let mut map_path = PathBuf::from(&output_dir);
                map_path.push("json/");
                fs.create_dir(&map_path).await?;
                map_path.push(format!(
                    "{}.json",
                    file_path.file_stem().unwrap().to_str().unwrap(),
                ));
                fs.write_file(&map_path, map.as_json().as_bytes().to_vec())
                    .await?;
                Ok(())
            });
        } else {
            let output = new_to_legacy(args.file.as_ref(), &io, &thread_pool).unwrap();
            let fs = io.fs.clone();
            let output_dir = args.output.clone();
            io.io_batcher.spawn_without_lifetime(async move {
                fs.create_dir(output_dir.as_ref()).await?;
                // write map
                let mut map_path = PathBuf::from(&output_dir);
                map_path.push("legacy/maps/");
                fs.create_dir(&map_path).await?;
                map_path.push(format!(
                    "{}.map",
                    file_path.file_stem().unwrap().to_str().unwrap(),
                ));
                fs.write_file(&map_path, output.map).await?;
                Ok(())
            });
        }
    }
}
