use std::path::Path;

use base_io::io::{Io, IoFileSys};
use game_config::config::ConfigGame;

pub fn save(config: &ConfigGame, io: &Io) {
    let save_str = config.to_json_string();

    if let Ok(save_str) = save_str {
        let fs_clone = io.fs.clone();
        io.io_batcher.spawn_without_lifetime(async move {
            fs_clone
                .write_file("cfg_game.json".as_ref(), save_str.as_bytes().to_vec())
                .await
                .unwrap();
            Ok(())
        });
    }
}

pub fn load_in(io: &IoFileSys, path: &Path) -> ConfigGame {
    let fs = io.fs.clone();
    let path = path.to_path_buf();
    let config_file = io
        .io_batcher
        .spawn(async move { Ok(fs.read_file(path.as_ref()).await) });
    let res = config_file.get_storage().unwrap();
    match res {
        Ok(file) => ConfigGame::from_json_string(String::from_utf8(file).unwrap().as_str())
            .unwrap_or_default(),
        Err(_) => ConfigGame::new(),
    }
}

pub fn load(io: &IoFileSys) -> ConfigGame {
    load_in(io, "cfg_game.json".as_ref())
}
