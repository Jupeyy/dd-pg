#![allow(clippy::too_many_arguments)]
#![allow(clippy::module_inception)]

pub mod client;

use std::sync::{atomic::AtomicBool, Arc};

use base::system::System;
use client::client::ddnet_main;
pub use client::*;
use native::native::app::NativeApp;
use network::network::utils::create_certifified_keys;
use server::server::ddnet_server_main;
use shared_base::network::server_info::ServerInfo;
use x509_cert::der::Encode;

#[cfg(feature = "alloc-track")]
#[global_allocator]
static GLOBAL_ALLOC: alloc_track::AllocTrack<std::alloc::System> =
    alloc_track::AllocTrack::new(std::alloc::System, alloc_track::BacktraceMode::Short);

fn main_impl(app: NativeApp) {
    let _ = thread_priority::set_current_thread_priority(thread_priority::ThreadPriority::Max);
    let sys = System::new();

    let (cert, private_key) = create_certifified_keys();
    let server_cert = cert.to_der().unwrap().to_vec();

    let server_is_open = Arc::new(AtomicBool::new(true));
    let server_is_open_clone = server_is_open.clone();

    let sys_clone = sys.clone();

    let shared_info: Arc<ServerInfo> = Arc::new(ServerInfo::new(true));
    let shared_info_thread = shared_info.clone();
    let t = std::thread::Builder::new()
        .name("server".into())
        .spawn(move || {
            ddnet_server_main::<true>(
                sys_clone,
                (cert, private_key),
                server_is_open_clone,
                shared_info_thread,
                None,
            )
        })
        .unwrap();

    let mut args: Vec<_> = std::env::args().collect();
    // TODO: don't rely on first arg being executable
    if !args.is_empty() {
        args.remove(0);
    }
    if let Err(err) = ddnet_main(args, sys, server_cert.as_slice(), shared_info, app) {
        panic!("exited client with an error: {err}"); // TODO: panic or graceful closing?
    }
    server_is_open.store(false, std::sync::atomic::Ordering::Relaxed);
    if let Err(_err) = t.join() {
        // TODO?
    }
}

#[allow(dead_code)]
fn main() {
    if std::env::var("RUST_LOG").is_err() {
        unsafe { std::env::set_var("RUST_LOG", "info,symphonia=warn,df::tract=error") };
    }
    env_logger::init();
    #[cfg(not(target_os = "android"))]
    main_impl(())
}

#[allow(dead_code)]
#[cfg(target_os = "android")]
#[no_mangle]
fn android_main(app: NativeApp) {
    // Get the application's internal storage directory
    let app_dir = app
        .external_data_path()
        .ok_or("Failed to get the external data path")
        .unwrap()
        .to_path_buf();

    // Set the current directory to the app's directory
    std::env::set_current_dir(&app_dir).unwrap();

    use log::LevelFilter;

    android_logger::init_once(android_logger::Config::default().with_max_level(LevelFilter::Trace));
    dbg!(app_dir);
    main_impl(app)
}
