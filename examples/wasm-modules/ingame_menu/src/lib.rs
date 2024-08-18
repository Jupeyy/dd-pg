#![allow(unused, unused_imports)]

pub mod ingame_menu;

use api::{graphics::graphics::GraphicsBackend, GRAPHICS};
use game_config::config::Config;
use ui_traits::traits::UiPageInterface;

pub use api_ui::ui_impl::*;
pub use api_ui_game::render::*;

#[no_mangle]
fn mod_ui_new() -> Box<dyn UiPageInterface<Config>> {
    Box::new(ingame_menu::page::IngameMenu::new(unsafe {
        &GRAPHICS.borrow()
    }))
}
