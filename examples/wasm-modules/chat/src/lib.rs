#![allow(clippy::all)]
#![allow(unused)]

pub mod chat;

use api::graphics::graphics::GraphicsBackend;
use ui_traits::traits::UIRenderCallbackFunc;

pub use api_ui::ui_impl::*;
pub use api_ui_game::render::*;

#[no_mangle]
fn mod_ui_new() -> Box<dyn UIRenderCallbackFunc<()>> {
    Box::new(chat::page::ChatPage::new())
}
