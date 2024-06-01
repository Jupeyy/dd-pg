#![allow(clippy::all)]

use std::time::Duration;

pub use api::*;
pub use api_render_game::*;
use client_render_game::render_game::{RenderGame, RenderGameInterface};
use config::config::ConfigEngine;
use url::Url;

#[no_mangle]
fn mod_render_game_new(
    map_file: Vec<u8>,
    resource_download_server: Option<Url>,
    config: &ConfigEngine,
) -> Box<dyn RenderGameInterface> {
    let state = RenderGame::new(
        unsafe { &*SOUND.borrow() },
        unsafe { &*GRAPHICS.borrow() },
        unsafe { &*IO.borrow() },
        &*RUNTIME_THREAD_POOL,
        &Duration::ZERO,
        map_file,
        resource_download_server,
        config,
    );
    Box::new(state)
}
