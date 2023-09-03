use api::graphics::graphics::{Graphics, GraphicsBackend};
use api_ui::UIWinitWrapper;
use graphics_base::streaming::{DrawScopeImpl, GraphicsStreamHandleInterface};
use graphics_types::types::CQuadItem;
use ui_base::types::{UIPipe, UIState};
use ui_traits::traits::UIRenderCallbackFunc;

pub struct ExamplePage {}

impl ExamplePage {
    pub fn new() -> Self {
        Self {}
    }
}

impl UIRenderCallbackFunc<UIWinitWrapper, GraphicsBackend> for ExamplePage {
    fn render(
        &mut self,
        ui: &mut egui::Ui,
        _pipe: &mut UIPipe<UIWinitWrapper>,
        _ui_state: &mut UIState<UIWinitWrapper>,
        graphics: &mut Graphics,
    ) {
        if ui.button("I'm a example page...").clicked() {
            // do nothing
        }

        let mut quad_scope = graphics.quads_begin();
        quad_scope.map_canvas(0.0, 0.0, 200.0, 200.0);
        quad_scope.set_colors_from_single(1.0, 0.0, 0.0, 0.25);
        quad_scope.quads_draw_tl(&[CQuadItem::new(50.0, 0.0, 100.0, 100.0)]);
    }
}
