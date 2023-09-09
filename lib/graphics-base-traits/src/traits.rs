use std::{cell::RefCell, fmt::Debug, rc::Rc};

use graphics_types::{
    command_buffer::AllCommands,
    rendering::SVertex,
    types::{GraphicsBackendMemory, GraphicsMemoryAllocationType, WindowProps},
};

pub trait GraphicsStreamDataInterface: Debug {
    fn vertices(&self) -> &[SVertex];
    fn vertices_mut(&mut self) -> &mut [SVertex];
    fn vertices_count(&self) -> usize;
    fn vertices_count_mut(&mut self) -> &mut usize;
    fn vertices_and_count(&self) -> (&[SVertex], &usize);
    fn vertices_and_count_mut(&mut self) -> (&mut [SVertex], &mut usize);

    fn set_vertices_unsafe(&mut self, vertices: &'static mut [SVertex]);
}

pub trait GraphicsBackendHandleInterface {
    fn run_backend_buffer(&mut self, stream_data: &Rc<RefCell<dyn GraphicsStreamDataInterface>>);

    fn add_cmd(&self, cmd: AllCommands);

    fn mem_alloc(
        &mut self,
        alloc_type: GraphicsMemoryAllocationType,
        req_size: usize,
    ) -> GraphicsBackendMemory;
}

pub trait GraphicsSizeQuery {
    fn canvas_aspect(&self) -> f32;
    fn canvas_width(&self) -> u32;
    fn canvas_height(&self) -> u32;
    fn window_width(&self) -> u32;
    fn window_height(&self) -> u32;

    fn window_props(&self) -> WindowProps;
}
