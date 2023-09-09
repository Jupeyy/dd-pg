use graphics_backend_traits::traits::GraphicsBackendInterface;

use super::graphics::GraphicsBase;

pub struct WindowEventPipe<'a, B: GraphicsBackendInterface> {
    pub graphics: &'a mut GraphicsBase<B>,
}

pub struct WindowHandling<'a, B: GraphicsBackendInterface> {
    pub pipe: WindowEventPipe<'a, B>,
}
