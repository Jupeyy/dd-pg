// TODO! ^ remove this
#![warn(clippy::perf)]
#![forbid(unsafe_code)]
//#![warn(missing_docs)]
#![warn(warnings)]
#![warn(clippy::nursery)]
#![warn(clippy::pedantic)]
#![warn(clippy::all)]
pub mod client;

use std::sync::{atomic::AtomicBool, Arc};

use ::network::network::quinnminimal::create_certificate;
use base::system::System;
use client::client::ddnet_main;
pub use client::*;
use server::server::{ddnet_server_main, ServerInfo};

fn main() {
    let cert = create_certificate();
    let server_cert = cert.serialize_der().unwrap();

    let server_is_open = Arc::new(AtomicBool::new(true));
    let server_is_open_clone = server_is_open.clone();

    let sys = System::new();
    let sys_clone = sys.clone();

    let shared_info = Arc::new(ServerInfo {
        sock_addr: std::sync::Mutex::new(None),
    });
    let shared_info_thread = shared_info.clone();
    let t = std::thread::spawn(move || {
        ddnet_server_main::<true>(sys_clone, &cert, server_is_open_clone, shared_info_thread)
    });
    ddnet_main(sys, server_cert.as_slice(), shared_info);
    server_is_open.store(false, std::sync::atomic::Ordering::Relaxed);
    if let Err(_err) = t.join() {
        // TODO?
    }
}
