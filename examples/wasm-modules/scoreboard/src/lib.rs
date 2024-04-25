#![allow(clippy::all)]
#![allow(unused, unused_imports)]

pub mod scoreboard;

use api::{graphics::graphics::GraphicsBackend, GRAPHICS};
use ui_traits::traits::UIRenderCallbackFunc;

pub use api_ui::ui_impl::*;
pub use api_ui_game::render::*;

#[no_mangle]
fn mod_ui_new() -> Box<dyn UIRenderCallbackFunc<()>> {
    Box::new(scoreboard::page::Scoreboard::new(unsafe {
        &GRAPHICS.borrow()
    }))
}
