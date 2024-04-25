#![allow(clippy::all)]
#![allow(unused, unused_imports)]

pub mod mainmenu;

use api::graphics::graphics::GraphicsBackend;
use game_config::config::Config;
use ui_traits::traits::UIRenderCallbackFunc;

pub use api_ui::ui_impl::*;
pub use api_ui_game::render::*;

#[no_mangle]
fn mod_ui_new() -> Box<dyn UIRenderCallbackFunc<Config>> {
    Box::new(mainmenu::page::MainMenu::new())
}
