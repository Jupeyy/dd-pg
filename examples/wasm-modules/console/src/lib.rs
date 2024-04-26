#![allow(clippy::all)]
#![allow(unused, unused_imports)]

pub mod console;

use api::graphics::graphics::GraphicsBackend;
use ui_traits::traits::UIRenderCallbackFunc;

pub use api_ui::ui_impl::*;
pub use api_ui_game::render::*;

#[no_mangle]
fn mod_ui_new() -> Box<dyn UIRenderCallbackFunc<()>> {
    Box::new(console::page::Console::new())
}
