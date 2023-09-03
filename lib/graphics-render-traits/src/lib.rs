use graphics_base::{
    buffer_object_handle::GraphicsBufferObjectHandleInterface,
    quad_container::GraphicsQuadContainerHandleInterface, streaming::GraphicsStreamHandleInterface,
};
use graphics_base_traits::traits::GraphicsBackendHandleInterface;

pub struct GraphicsHandles<'a> {
    pub backend_handle: &'a mut dyn GraphicsBackendHandleInterface,
    pub stream_handle: &'a mut dyn GraphicsStreamHandleInterface,
    pub quad_container_handle: &'a mut dyn GraphicsQuadContainerHandleInterface,
    pub buffer_object_handle: &'a mut dyn GraphicsBufferObjectHandleInterface,
}

pub trait GraphicsHandlesInterface {
    fn get_handles(&mut self) -> GraphicsHandles;
}
