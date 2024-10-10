use std::sync::{atomic::AtomicBool, Arc};

use base::system::System;
use clap::{arg, command, Command};
use game_config::config::ConfigGame;
use network::network::utils::create_certifified_keys;
use server::server::ddnet_server_main;
use shared_base::network::server_info::ServerInfo;

fn main() {
    let matches = command!()
        .subcommand(Command::new("default_config").about("Print the default config"))
        .arg(
            arg!(-c --config <cfg> "A relative path to a config file, used instead of cfg_game.json."),
        )
        .get_matches();

    let sys = System::new();
    if std::env::var("RUST_LOG").is_err() {
        unsafe { std::env::set_var("RUST_LOG", "info") };
    }
    env_logger::init();

    if matches
        .subcommand_name()
        .is_some_and(|cmd| cmd == "default_config")
    {
        println!(
            "{}",
            serde_json::to_string_pretty(&ConfigGame::default()).unwrap()
        );
        return;
    }

    let cfg_game = matches.get_one::<String>("config");

    let cert = create_certifified_keys();

    let server_is_open = Arc::new(AtomicBool::new(true));
    let server_is_open_clone = server_is_open.clone();

    let sys_clone = sys.clone();

    let shared_info = Arc::new(ServerInfo::new(false));
    ddnet_server_main::<false>(
        sys_clone,
        cert,
        server_is_open_clone,
        shared_info,
        cfg_game.map(|p| p.as_ref()),
    );
    server_is_open.store(false, std::sync::atomic::Ordering::Relaxed);
}
