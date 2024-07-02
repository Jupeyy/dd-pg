use std::sync::{atomic::AtomicBool, Arc};

use base::system::System;
use network::network::utils::create_certifified_keys;
use server::server::ddnet_server_main;
use shared_base::network::server_info::ServerInfo;

fn main() {
    let cert = create_certifified_keys();

    let server_is_open = Arc::new(AtomicBool::new(true));
    let server_is_open_clone = server_is_open.clone();

    let sys = System::new();
    let sys_clone = sys.clone();

    let shared_info = Arc::new(ServerInfo::new(true));
    ddnet_server_main::<false>(sys_clone, cert, server_is_open_clone, shared_info);
    server_is_open.store(false, std::sync::atomic::Ordering::Relaxed);
}
