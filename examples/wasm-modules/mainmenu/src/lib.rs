pub mod mainmenu;

use api::{GRAPHICS, IO};
use ui_traits::traits::UiPageInterface;

pub use api_ui::ui_impl::*;
pub use api_ui_game::render::*;

#[no_mangle]
fn mod_ui_new() -> Box<dyn UiPageInterface<()>> {
    Box::new(mainmenu::page::MainMenu::new(
        unsafe { &GRAPHICS.borrow() },
        unsafe { IO.borrow().clone() },
    ))
}
