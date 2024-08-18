use std::{sync::Arc, time::Duration};

pub use api::*;
pub use api_render_game::*;
use client_render_game::render_game::{RenderGame, RenderGameInterface};
use config::config::ConfigEngine;
use ui_base::font_data::UiFontData;
use url::Url;

#[no_mangle]
fn mod_render_game_new(
    map_file: Vec<u8>,
    resource_download_server: Option<Url>,
    config: &ConfigEngine,
    fonts: Arc<UiFontData>,
) -> Box<dyn RenderGameInterface> {
    let state = RenderGame::new(
        unsafe { &SOUND.borrow() },
        unsafe { &GRAPHICS.borrow() },
        unsafe { &IO.borrow() },
        &RUNTIME_THREAD_POOL,
        &Duration::ZERO,
        map_file,
        resource_download_server,
        config,
        fonts,
    );
    Box::new(state)
}
