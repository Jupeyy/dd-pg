use graphics::graphics::GraphicsBase;
use graphics_backend_traits::traits::GraphicsBackendInterface;
use ui_base::types::{UIPipe, UIState};

pub trait UIRenderCallbackFunc<U, B>
where
    B: GraphicsBackendInterface,
{
    /// returns true, if the main frame should be rendered and should be used
    /// for post processing effects like blur
    #[must_use]
    fn has_blur(&self) -> bool;

    /// only used for post processing effects like blur
    fn render_main_frame(
        &mut self,
        ui: &mut egui::Ui,
        pipe: &mut UIPipe<U>,
        ui_state: &mut UIState,
        graphics: &mut GraphicsBase<B>,
    );

    fn render(
        &mut self,
        ui: &mut egui::Ui,
        pipe: &mut UIPipe<U>,
        ui_state: &mut UIState,
        graphics: &mut GraphicsBase<B>,
    );
}
