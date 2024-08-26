#![allow(unused)]

pub mod pred_timer_ui;

use api::{graphics::graphics::GraphicsBackend, GRAPHICS};
use ui_traits::traits::UiPageInterface;

pub use api_ui::ui_impl::*;
pub use api_ui_game::render::*;

#[no_mangle]
fn mod_ui_new() -> Box<dyn UiPageInterface<()>> {
    Box::new(pred_timer_ui::page::PredTimerPage::new(unsafe {
        &GRAPHICS.borrow()
    }))
}
