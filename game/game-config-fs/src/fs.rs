use base_io::io::{IOFileSys, IO};
use game_config::config::ConfigGame;

pub fn save(config: &ConfigGame, io: &IO) {
    let save_str = config.to_json_string();

    if let Ok(save_str) = save_str {
        let fs_clone = io.fs.clone();
        let _ = io.io_batcher.spawn_without_queue(async move {
            fs_clone
                .write_file("cfg_game.json", save_str.as_bytes().to_vec())
                .await
                .unwrap();
            Ok(())
        });
    }
}

pub fn load(io: &IOFileSys) -> ConfigGame {
    let fs = io.fs.clone();
    let config_file = io
        .io_batcher
        .spawn(async move { Ok(fs.open_file("cfg_game.json").await) });
    let res = config_file.get_storage().unwrap();
    match res {
        Ok(file) => ConfigGame::from_json_string(String::from_utf8(file).unwrap().as_str())
            .unwrap_or_default(),
        Err(_) => ConfigGame::new(),
    }
}
