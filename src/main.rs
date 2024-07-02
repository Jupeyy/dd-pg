#![allow(clippy::all)]

pub mod client;

use std::sync::{atomic::AtomicBool, Arc};

use base::system::System;
use client::client::ddnet_main;
pub use client::*;
use network::network::utils::create_certifified_keys;
use server::server::ddnet_server_main;
use shared_base::network::server_info::ServerInfo;
use x509_cert::der::Encode;

fn main() {
    let _ = thread_priority::set_current_thread_priority(thread_priority::ThreadPriority::Max);
    std::env::set_var("RUST_LOG", "info");
    env_logger::init();
    let sys = System::new();

    let (cert, private_key) = create_certifified_keys();
    let server_cert = cert.to_der().unwrap().to_vec();

    let server_is_open = Arc::new(AtomicBool::new(true));
    let server_is_open_clone = server_is_open.clone();

    let sys_clone = sys.clone();

    let shared_info: Arc<ServerInfo> = Default::default();
    let shared_info_thread = shared_info.clone();
    let t = std::thread::Builder::new()
        .name("server".into())
        .spawn(move || {
            ddnet_server_main::<true>(
                sys_clone,
                (cert, private_key),
                server_is_open_clone,
                shared_info_thread,
            )
        })
        .unwrap();

    let mut args: Vec<_> = std::env::args().collect();
    // TODO: don't rely on first arg being executable
    if !args.is_empty() {
        args.remove(0);
    }
    if let Err(err) = ddnet_main(args, sys, server_cert.as_slice(), shared_info) {
        panic!("exited client with an error: {err}"); // TODO: panic or graceful closing?
    }
    server_is_open.store(false, std::sync::atomic::Ordering::Relaxed);
    if let Err(_err) = t.join() {
        // TODO?
    }
}
