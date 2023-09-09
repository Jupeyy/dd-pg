use graphics::graphics::GraphicsBase;
use graphics_backend_traits::traits::GraphicsBackendInterface;
use ui_base::types::{UIPipe, UIState};

pub trait UIRenderCallbackFunc<U, B>
where
    B: GraphicsBackendInterface,
{
    fn render(
        &mut self,
        ui: &mut egui::Ui,
        pipe: &mut UIPipe<U>,
        ui_state: &mut UIState<U>,
        graphics: &mut GraphicsBase<B>,
    );
}
