#![deny(warnings)]
#![deny(clippy::all)]

pub mod map;
pub mod skeleton;
pub mod types;
pub mod utils;

#[cfg(test)]
mod test {
    use std::{
        io::{Read, Write},
        sync::Arc,
    };

    use base::{benchmark::Benchmark, system::System};
    use base_fs::filesys::FileSystem;
    use base_io::io::IOFileSys;
    use flate2::Compression;

    use crate::map::{groups::MapGroup, Map};

    fn compression_tests_for_map(map_name: &str) {
        let workspace_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../");
        std::env::set_current_dir(workspace_root).unwrap();
        let sys = System::new();
        let io = IOFileSys::new(Arc::new(FileSystem::new(&sys.log, "org", "", "DDNet")));

        let thread_pool = Arc::new(
            rayon::ThreadPoolBuilder::new()
                .num_threads(1)
                .build()
                .unwrap(),
        );

        let fs = io.fs.clone();
        let tp = thread_pool.clone();
        let map_name = map_name.to_string();
        let map_legacy = io.io_batcher.spawn(async move {
            let map = fs
                .open_file(format!("map/maps/{}.twmap", map_name).as_ref())
                .await?;

            Map::read(&map, &tp)
        });
        let map = map_legacy.get_storage().unwrap();

        let benchmark = Benchmark::new(true);
        let groups_encoded =
            bincode::serde::encode_to_vec(map.groups.foreground, bincode::config::standard())
                .unwrap();
        benchmark.bench("encoding (bincode)");
        let _ = bincode::serde::decode_from_slice::<Vec<MapGroup>, _>(
            &groups_encoded,
            bincode::config::standard(),
        );
        benchmark.bench("decode (bincode)");

        fn compression_of_group(groups_encoded: Vec<u8>, benchmark: &Benchmark) {
            let mut writer: Vec<u8> = Default::default();
            flate2::write::DeflateEncoder::new(&mut writer, Compression::default())
                .write_all(&groups_encoded)
                .unwrap();
            let len_deflate_default = writer.len();
            writer.clear();
            benchmark.bench("deflate default");

            flate2::write::DeflateEncoder::new(&mut writer, Compression::new(9))
                .write_all(&groups_encoded)
                .unwrap();
            let len_deflate_best = writer.len();
            benchmark.bench("deflate best");
            flate2::read::DeflateDecoder::new(writer.clone().as_slice())
                .read_to_end(&mut writer)
                .unwrap();
            writer.clear();
            benchmark.bench("deflate decompress");

            flate2::write::GzEncoder::new(&mut writer, Compression::default())
                .write_all(&groups_encoded)
                .unwrap();
            let len_gz_default = writer.len();
            writer.clear();
            benchmark.bench("gz default");

            flate2::write::GzEncoder::new(&mut writer, Compression::new(9))
                .write_all(&groups_encoded)
                .unwrap();
            let len_gz_best = writer.len();
            benchmark.bench("gz best");
            flate2::read::GzDecoder::new(writer.clone().as_slice())
                .read_to_end(&mut writer)
                .unwrap();
            writer.clear();
            benchmark.bench("gz decompress");

            flate2::write::ZlibEncoder::new(&mut writer, Compression::default())
                .write_all(&groups_encoded)
                .unwrap();
            let len_zlib_default = writer.len();
            writer.clear();
            benchmark.bench("zlib default");

            flate2::write::ZlibEncoder::new(&mut writer, Compression::new(9))
                .write_all(&groups_encoded)
                .unwrap();
            let len_zlib_best = writer.len();
            benchmark.bench("zlib best");
            flate2::read::ZlibDecoder::new(writer.clone().as_slice())
                .read_to_end(&mut writer)
                .unwrap();
            writer.clear();
            benchmark.bench("zlib decompress");

            let mut writer = lz4_flex::block::compress(&groups_encoded);
            let len_lz4 = writer.len();
            benchmark.bench("lz4 best");
            let _ = lz4_flex::block::decompress(&writer, groups_encoded.len()).unwrap();
            writer.clear();
            benchmark.bench("lz4 decompress");

            brotli::CompressorWriter::new(&mut writer, 4096, 9, 22)
                .write_all(&groups_encoded)
                .unwrap();
            let len_brotli_best = writer.len();
            benchmark.bench("brotli best(suggestion)");
            brotli::Decompressor::new(writer.clone().as_slice(), 4096)
                .read_to_end(&mut writer)
                .unwrap();
            writer.clear();
            benchmark.bench("brotli best decompress");

            brotli::CompressorWriter::new(&mut writer, 4096, 8, 22)
                .write_all(&groups_encoded)
                .unwrap();
            let len_brotli_8 = writer.len();
            benchmark.bench("brotli 8");
            brotli::Decompressor::new(writer.clone().as_slice(), 4096)
                .read_to_end(&mut writer)
                .unwrap();
            writer.clear();
            benchmark.bench("brotli 8 decompress");

            brotli::CompressorWriter::new(&mut writer, 4096, 6, 22)
                .write_all(&groups_encoded)
                .unwrap();
            let len_brotli_6 = writer.len();
            benchmark.bench("brotli 6");
            brotli::Decompressor::new(writer.clone().as_slice(), 4096)
                .read_to_end(&mut writer)
                .unwrap();
            writer.clear();
            benchmark.bench("brotli 6 decompress");

            brotli::CompressorWriter::new(&mut writer, 4096, 3, 22)
                .write_all(&groups_encoded)
                .unwrap();
            let len_brotli_3 = writer.len();
            benchmark.bench("brotli 3");
            brotli::Decompressor::new(writer.clone().as_slice(), 4096)
                .read_to_end(&mut writer)
                .unwrap();
            writer.clear();
            benchmark.bench("brotli 3 decompress");

            zstd::stream::copy_encode(groups_encoded.as_slice(), &mut writer, 3).unwrap();
            let len_zstd_default = writer.len();
            writer.clear();
            benchmark.bench("zstd default");

            zstd::stream::copy_encode(groups_encoded.as_slice(), &mut writer, 9).unwrap();
            let len_zstd_best = writer.len();
            benchmark.bench("zstd best");
            let _ = zstd::stream::decode_all(writer.clone().as_slice()).unwrap();
            writer.clear();
            benchmark.bench("zstd decompress");

            lzma::LzmaWriter::new_compressor(&mut writer, 6)
                .unwrap()
                .write_all(groups_encoded.as_slice())
                .unwrap();
            let len_lzma_default = writer.len();
            writer.clear();
            benchmark.bench("lzma default");

            lzma::LzmaWriter::new_compressor(&mut writer, 8)
                .unwrap()
                .write_all(groups_encoded.as_slice())
                .unwrap();
            let len_lzma_best = writer.len();
            benchmark.bench("lzma best");
            /*lzma::LzmaReader::new_decompressor(writer.clone().as_slice())
            .unwrap()
            .read_to_end(&mut writer)
            .unwrap();*/
            writer.clear();
            benchmark.bench("lzma decompress (broken?)");

            println!(
            "uncompressed: {}, deflate: {} - {}, gz: {} - {}, zlib: {} - {}, lz4: {}, brotli: {} - {} - {} - {}, zstd: {} - {}, lzma: {} - {}",
            groups_encoded.len(),
            len_deflate_default,
            len_deflate_best,
            len_gz_default,
            len_gz_best,
            len_zlib_default,
            len_zlib_best,
            len_lz4,
            len_brotli_best,
            len_brotli_8,
            len_brotli_6,
            len_brotli_3,
            len_zstd_default,
            len_zstd_best,
            len_lzma_default,
            len_lzma_best
        );
        }
        compression_of_group(groups_encoded, &benchmark);
    }

    /// some tests to evaluate best compression
    #[test]
    fn compression_tests() {
        compression_tests_for_map("ctf1");
        compression_tests_for_map("arctic");
    }
}
