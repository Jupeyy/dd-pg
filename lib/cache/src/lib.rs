use std::sync::{Arc, Weak};

use base_fs::filesys::{FileSystem, FileSystemPath, FileSystemType};
use sha3::{
    digest::{generic_array::GenericArray, typenum::U32},
    Digest, Keccak256,
};
use tokio::sync::RwLock;

struct CacheImpl {
    cache_name: String,
    hasher: Keccak256,
    // only keep a weak reference for now, so
    // caches have to be destroyed before the fs
    fs: Weak<FileSystem>,
}

/**
 * Make it easy to cache computational expensive processes
 * that result in serializable data with ddnet's filesystem
 * This is a pure filesystem wrapper and does not hold any states,
 * except the ones required to make sure only one cache item is
 * computed at one time
 */
pub struct Cache<const VERSION: usize> {
    cache: RwLock<CacheImpl>,
}

impl<const VERSION: usize> Cache<{ VERSION }> {
    pub fn new(cache_name: &str, fs: &Arc<FileSystem>) -> Self {
        Self {
            cache: RwLock::new(CacheImpl {
                cache_name: cache_name.to_string(),
                hasher: Keccak256::new(),
                fs: Arc::downgrade(fs),
            }),
        }
    }

    pub async fn load_from_binary<F>(
        &self,
        original_binary_data: &[u8],
        compute_func: F,
    ) -> anyhow::Result<Vec<u8>>
    where
        F: FnOnce(&[u8]) -> anyhow::Result<Vec<u8>>,
    {
        let mut cache = self.cache.write().await;
        cache.hasher.update(&original_binary_data[..]);
        let mut hash: GenericArray<u8, U32> = Default::default();
        cache.hasher.finalize_into_reset(&mut hash);
        let dir_name = "cache/".to_string() + &cache.cache_name + "/v" + &({ VERSION }.to_string());
        let hash_path = dir_name.clone() + "/f_" + &format!("{:X}", hash) + ".cached";
        let file = cache
            .fs
            .upgrade()
            .unwrap()
            .open_file_in(
                &hash_path,
                FileSystemPath::OfType(FileSystemType::ReadWrite),
            )
            .await;
        match file {
            Ok(cached_file) => Ok(cached_file),
            Err(_) => {
                if let Err(err) = cache.fs.upgrade().unwrap().create_dir(&dir_name).await {
                    todo!("{}", err);
                }
                let cached_result = compute_func(original_binary_data)?;
                if let Err(err) = cache
                    .fs
                    .upgrade()
                    .unwrap()
                    .write_file(&hash_path, cached_result.clone())
                    .await
                {
                    todo!("{}", err);
                }
                Ok(cached_result)
            }
        }
    }

    pub async fn load<F>(
        &self,
        original_file_path: &str,
        compute_func: F,
    ) -> anyhow::Result<Vec<u8>>
    where
        F: FnOnce(&[u8]) -> anyhow::Result<Vec<u8>>,
    {
        let cache = self.cache.read().await;
        let file = cache
            .fs
            .upgrade()
            .unwrap()
            .open_file(original_file_path)
            .await?;
        drop(cache);
        self.load_from_binary(&file, compute_func).await
    }
}
