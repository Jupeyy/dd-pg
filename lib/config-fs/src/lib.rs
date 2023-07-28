use std::sync::{Arc, Mutex};

use base_fs::{filesys::FileSystem, io_batcher::TokIOBatcher};
use config::config::Config;

pub fn save(config: &Config, fs: &Arc<FileSystem>, io_batcher: &Arc<Mutex<TokIOBatcher>>) {
    let save_str = config.to_json_string();

    if let Ok(save_str) = save_str {
        let fs_clone = fs.clone();
        io_batcher.lock().unwrap().spawn_without_queue(async move {
            fs_clone
                .write_file("config.json", save_str.as_bytes().to_vec())
                .await
                .unwrap();
            Ok(())
        });
    }
}

pub fn load(fs: &Arc<FileSystem>, batcher: &Arc<Mutex<TokIOBatcher>>) -> Config {
    let fs = fs.clone();
    let mut batcher = batcher.lock().unwrap();
    let mut config_file = batcher.spawn(async move { Ok(fs.open_file("config.json").await) });
    batcher.wait_finished_and_drop(&mut config_file);
    let res = config_file.get_storage().unwrap();
    match res {
        Ok(file) => Config::from_json_string(String::from_utf8(file).unwrap().as_str())
            .unwrap_or(Config::new()),
        Err(_) => Config::new(),
    }
}
