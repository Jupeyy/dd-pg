#![allow(unused, unused_imports)]

pub mod console;

use api::graphics::graphics::GraphicsBackend;
use ui_traits::traits::UiPageInterface;

pub use api_ui::ui_impl::*;
pub use api_ui_game::render::*;

#[no_mangle]
fn mod_ui_new() -> Box<dyn UiPageInterface<()>> {
    Box::new(console::page::Console::new())
}
