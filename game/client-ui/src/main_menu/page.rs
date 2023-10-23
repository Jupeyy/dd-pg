use graphics::graphics::GraphicsBase;
use graphics_backend_traits::traits::GraphicsBackendInterface;
use ui_base::types::{UIPipe, UIState};
use ui_traits::traits::UIRenderCallbackFunc;

use super::{main_frame, user_data::UserData};

pub struct ExamplePage {}

impl ExamplePage {
    pub fn new() -> Self {
        Self {}
    }
}

impl<'a, B: GraphicsBackendInterface> UIRenderCallbackFunc<UserData<'a>, B> for ExamplePage {
    fn has_blur(&self) -> bool {
        true
    }

    fn render_main_frame(
        &mut self,
        ui: &mut egui::Ui,
        pipe: &mut UIPipe<UserData>,
        ui_state: &mut UIState,
        graphics: &mut GraphicsBase<B>,
    ) {
        main_frame::render(ui, pipe, ui_state, graphics, true);
    }

    fn render(
        &mut self,
        ui: &mut egui::Ui,
        pipe: &mut UIPipe<UserData>,
        ui_state: &mut UIState,
        graphics: &mut GraphicsBase<B>,
    ) {
        main_frame::render(ui, pipe, ui_state, graphics, false)
    }
}
