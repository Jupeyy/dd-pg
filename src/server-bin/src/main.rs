use std::sync::{atomic::AtomicBool, Arc};

use base::system::System;
use clap::{command, Command};
use game_config::config::ConfigGame;
use network::network::utils::create_certifified_keys;
use server::server::ddnet_server_main;
use shared_base::network::server_info::ServerInfo;

fn main() {
    let matches = command!()
        .subcommand(Command::new("config").about("Print the default config"))
        .get_matches();

    let sys = System::new();
    unsafe { std::env::set_var("RUST_LOG", "info") };
    env_logger::init();

    if matches.subcommand_name().is_some_and(|cmd| cmd == "config") {
        println!(
            "{}",
            serde_json::to_string_pretty(&ConfigGame::default()).unwrap()
        );
        return;
    }

    let cert = create_certifified_keys();

    let server_is_open = Arc::new(AtomicBool::new(true));
    let server_is_open_clone = server_is_open.clone();

    let sys_clone = sys.clone();

    let shared_info = Arc::new(ServerInfo::new(false));
    ddnet_server_main::<false>(sys_clone, cert, server_is_open_clone, shared_info);
    server_is_open.store(false, std::sync::atomic::Ordering::Relaxed);
}
