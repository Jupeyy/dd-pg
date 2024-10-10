use std::time::Duration;

pub use api::*;
pub use api_render_game::*;
use client_render_game::render_game::{RenderGame, RenderGameCreateOptions, RenderGameInterface};
use config::config::ConfigDebug;

#[no_mangle]
fn mod_render_game_new(
    map_file: Vec<u8>,
    config: &ConfigDebug,
    props: RenderGameCreateOptions,
) -> Box<dyn RenderGameInterface> {
    let state = RenderGame::new(
        unsafe { &SOUND.borrow() },
        unsafe { &GRAPHICS.borrow() },
        unsafe { &IO.borrow() },
        &RUNTIME_THREAD_POOL,
        &Duration::ZERO,
        map_file,
        config,
        props,
    );
    Box::new(state)
}
