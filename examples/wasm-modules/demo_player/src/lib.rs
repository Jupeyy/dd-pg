pub mod demo_player;

use api::GRAPHICS;
use ui_traits::traits::UiPageInterface;

pub use api_ui::ui_impl::*;
pub use api_ui_game::render::*;

#[no_mangle]
fn mod_ui_new() -> Box<dyn UiPageInterface<()>> {
    Box::new(demo_player::page::DemoPlayerPage::new(unsafe {
        &GRAPHICS.borrow()
    }))
}
