#![allow(clippy::all)]

use std::{sync::Arc, time::Duration};

pub use api::*;
pub use api_render_game::*;
use base_log::log::SystemLog;
use client_render_game::render_game::{RenderGame, RenderGameInterface};
use config::config::ConfigEngine;

#[no_mangle]
fn mod_render_game_new(
    map_file: Vec<u8>,
    config: &ConfigEngine,
) -> Box<dyn RenderGameInterface> {
    let sys_log = Arc::new(SystemLog::new());
    let state = RenderGame::new(
        unsafe { &*SOUND.borrow() },
        unsafe { &*GRAPHICS.borrow() },
        unsafe { &*IO.borrow() },
        &*RUNTIME_THREAD_POOL,
        &Duration::ZERO,
        &sys_log,
        map_file,
        config,
    );
    Box::new(state)
}
