pub mod page;

use api::graphics::graphics::GraphicsBackend;
use api_ui::UIWinitWrapper;
use page::ExamplePage;
use ui_traits::traits::UIRenderCallbackFunc;

#[no_mangle]
fn mod_ui_new() -> Box<dyn UIRenderCallbackFunc<UIWinitWrapper, GraphicsBackend>> {
    Box::new(ExamplePage::new())
}
