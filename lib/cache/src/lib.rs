use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use base::hash::{fmt_hash, generate_hash_for, generate_hash_for_multi, Hash};
use base_io_traits::fs_traits::{FileSystemInterface, FileSystemPath, FileSystemType};
use hiarc::Hiarc;

#[derive(Debug, Hiarc)]
struct CacheImpl {
    cache_name: String,
    #[hiarc_skip_unsafe]
    fs: Arc<dyn FileSystemInterface>,
}

/// Make it easy to cache computational expensive processes
/// that result in serializable data with ddnet's filesystem.
///
/// This is a pure filesystem wrapper and does not hold any states,
/// except the ones required to make sure only one cache item is
/// computed at one time
#[derive(Debug, Hiarc)]
pub struct Cache<const VERSION: usize> {
    cache: CacheImpl,
}

impl<const VERSION: usize> Cache<{ VERSION }> {
    pub fn new(cache_name: &str, fs: &Arc<dyn FileSystemInterface>) -> Self {
        Self {
            cache: CacheImpl {
                cache_name: cache_name.to_string(),
                fs: fs.clone(),
            },
        }
    }

    /// returns the dir and the full path
    fn cache_file_path(cache: &CacheImpl, hash: &Hash) -> (PathBuf, PathBuf) {
        let dir_name = Path::new("cache/")
            .join(Path::new(&cache.cache_name))
            .join(Path::new(&format!("v{}", VERSION)));
        let hash_path = dir_name.join(Path::new(&format!("f_{}.cached", fmt_hash(hash))));
        (dir_name, hash_path)
    }

    /// like [`Cache::load_from_binary`], but allows additional bytes to be
    /// respected for the hash function
    pub async fn load_from_binary_ex<F>(
        &self,
        original_binary_data: &[u8],
        additional_hash_bytes: &[u8],
        compute_func: F,
    ) -> anyhow::Result<Vec<u8>>
    where
        F: FnOnce(&[u8]) -> anyhow::Result<Vec<u8>>,
    {
        let cache = &self.cache;
        let hash = if !additional_hash_bytes.is_empty() {
            generate_hash_for_multi([original_binary_data, additional_hash_bytes].as_slice())
        } else {
            generate_hash_for_multi([original_binary_data].as_slice())
        };
        let (dir_name, hash_path) = Self::cache_file_path(cache, &hash);
        let file = cache
            .fs
            .read_file_in(
                &hash_path,
                FileSystemPath::OfType(FileSystemType::ReadWrite),
            )
            .await;
        match file {
            Ok(cached_file) => Ok(cached_file),
            Err(_) => {
                if let Err(err) = cache.fs.create_dir(&dir_name).await {
                    Err(err.into())
                } else {
                    let cached_result = compute_func(original_binary_data)?;
                    if let Err(err) = cache.fs.write_file(&hash_path, cached_result.clone()).await {
                        Err(err.into())
                    } else {
                        Ok(cached_result)
                    }
                }
            }
        }
    }

    /// Checks if an binary entry exist in the cache, if not
    /// a compute heavy function passed by the user is called.
    /// This allows to skip this calculation the next time
    /// this function is called.
    pub async fn load_from_binary<F>(
        &self,
        original_binary_data: &[u8],
        compute_func: F,
    ) -> anyhow::Result<Vec<u8>>
    where
        F: FnOnce(&[u8]) -> anyhow::Result<Vec<u8>>,
    {
        self.load_from_binary_ex(original_binary_data, &[], compute_func)
            .await
    }

    /// Checks if a file exist in the cache, if not
    /// a compute heavy function passed by the user is called.
    /// This allows to skip this calculation the next time
    /// this function is called.
    pub async fn load<F>(
        &self,
        original_file_path: &str,
        compute_func: F,
    ) -> anyhow::Result<Vec<u8>>
    where
        F: FnOnce(&[u8]) -> anyhow::Result<Vec<u8>>,
    {
        let cache = &self.cache;
        let file = cache.fs.read_file(original_file_path.as_ref()).await?;
        self.load_from_binary(&file, compute_func).await
    }

    /// Puts a given file into the cache as is, returns the path of the cached file.
    /// This can be used to simply cache a file against changes
    /// The file extension is kept.
    pub async fn archieve(
        &self,
        original_file_path: &Path,
        in_path: FileSystemPath,
    ) -> anyhow::Result<PathBuf> {
        let cache = &self.cache;
        let file = cache
            .fs
            .read_file_in(original_file_path.as_ref(), in_path)
            .await?;
        let hash = generate_hash_for(&file);
        let (dir_name, mut file_path) = Self::cache_file_path(cache, &hash);

        // archieving will keep the original file ending
        if let Some(ext) = original_file_path.extension() {
            file_path.set_extension(ext);
        }

        // create file
        if !cache.fs.file_exists(&file_path).await {
            cache.fs.create_dir(&dir_name).await?;
            cache.fs.write_file(&file_path, file).await?;
        }

        Ok(file_path)
    }
}
