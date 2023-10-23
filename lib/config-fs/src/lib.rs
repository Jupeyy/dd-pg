#![allow(clippy::all)]

use base_io::io::IO;
use config::config::Config;

pub fn save(config: &Config, io: &IO) {
    let save_str = config.to_json_string();

    if let Ok(save_str) = save_str {
        let fs_clone = io.fs.clone();
        io.io_batcher.spawn_without_queue(async move {
            fs_clone
                .write_file("config.json", save_str.as_bytes().to_vec())
                .await
                .unwrap();
            Ok(())
        });
    }
}

pub fn load(io: &IO) -> Config {
    let fs = io.fs.clone();
    let config_file = io
        .io_batcher
        .spawn(async move { Ok(fs.open_file("config.json").await) });
    let res = config_file.get_storage().unwrap();
    match res {
        Ok(file) => Config::from_json_string(String::from_utf8(file).unwrap().as_str())
            .unwrap_or(Config::new()),
        Err(_) => Config::new(),
    }
}
