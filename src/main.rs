#![allow(warnings, unused)]
//#![allow(unused)]
// TODO! ^ remove this
//#![warn(clippy::perf)]
//#![forbid(unsafe_code)]
//#![warn(missing_docs)]
//#![warn(warnings)]
//#![warn(clippy::nursery)]
//#![warn(clippy::pedantic)]
//#![warn(clippy::all)]

pub mod client;
pub mod server;
pub mod shared;

use std::sync::{atomic::AtomicBool, Arc};

use ::network::network::quinnminimal::create_certificate;
use base::system::System;
use client::client::ddnet_main;
pub use client::*;
use server::server::ddnet_server_main;
pub use server::*;
pub use shared::*;

fn main() {
    let cert = create_certificate();
    let server_cert = cert.serialize_der().unwrap();

    let server_is_open = Arc::new(AtomicBool::new(true));
    let server_is_open_clone = server_is_open.clone();

    let sys = System::new();
    let sys_clone = sys.clone();

    let t = std::thread::spawn(move || ddnet_server_main(sys_clone, &cert, server_is_open_clone));
    ddnet_main(sys, server_cert.as_slice());
    server_is_open.store(false, std::sync::atomic::Ordering::Relaxed);
    if let Err(_err) = t.join() {
        // TODO?
    }
}
