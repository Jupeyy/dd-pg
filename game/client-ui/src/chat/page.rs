use graphics::graphics::GraphicsBase;
use graphics_backend_traits::traits::GraphicsBackendInterface;
use ui_base::types::{UIPipe, UIState};
use ui_traits::traits::UIRenderCallbackFunc;

use super::main_frame;

pub struct ExamplePage {}

impl ExamplePage {
    pub fn new() -> Self {
        Self {}
    }
}

impl<B: GraphicsBackendInterface> UIRenderCallbackFunc<(), B> for ExamplePage {
    fn has_blur(&self) -> bool {
        true
    }

    fn render_main_frame(
        &mut self,
        ui: &mut egui::Ui,
        pipe: &mut UIPipe<()>,
        ui_state: &mut UIState,
        graphics: &mut GraphicsBase<B>,
    ) {
        main_frame::render(ui, pipe, ui_state, graphics);
    }

    fn render(
        &mut self,
        ui: &mut egui::Ui,
        pipe: &mut UIPipe<()>,
        ui_state: &mut UIState,
        graphics: &mut GraphicsBase<B>,
    ) {
        main_frame::render(ui, pipe, ui_state, graphics)
    }
}
