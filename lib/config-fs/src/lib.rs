use std::sync::Arc;

use base_fs::{filesys::FileSystem, io_batcher::TokIOBatcher};
use base_fs_traits::traits::FileSystemInterface;
use config::config::Config;

pub fn save(config: &Config, fs: &Arc<FileSystem>, io_batcher: &TokIOBatcher) {
    let save_str = config.to_json_string();

    if let Ok(save_str) = save_str {
        let fs_clone = fs.clone();
        io_batcher.spawn_without_queue(async move {
            fs_clone
                .write_file("config.json", save_str.as_bytes().to_vec())
                .await
                .unwrap();
            Ok(())
        });
    }
}

pub fn load(fs: &Arc<FileSystem>, batcher: &TokIOBatcher) -> Config {
    let fs = fs.clone();
    let config_file = batcher.spawn(async move { Ok(fs.open_file("config.json").await) });
    let res = config_file.get_storage().unwrap();
    match res {
        Ok(file) => Config::from_json_string(String::from_utf8(file).unwrap().as_str())
            .unwrap_or(Config::new()),
        Err(_) => Config::new(),
    }
}
