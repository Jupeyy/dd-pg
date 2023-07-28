use graphics_types::{
    command_buffer::AllCommands,
    rendering::{SVertex, State},
    types::DrawModes,
};

pub trait GraphicsBackendBufferInterface {
    fn vertices_mut(&mut self) -> &mut [SVertex];
    fn vertices_count(&self) -> usize;
    fn vertices_count_mut(&mut self) -> &mut usize;
    fn vertices_and_count_mut(&mut self) -> (&mut [SVertex], &mut usize);
}

pub trait GraphicsStreamHandler {
    fn backend_buffer_mut(&mut self) -> &mut dyn GraphicsBackendBufferInterface;

    fn flush_vertices(&mut self, state: &State, vertices_offset: usize, draw_mode: DrawModes);

    fn run_backend_buffer(&mut self);

    fn add_cmd(&mut self, cmd: AllCommands);
}

pub trait GraphicsSizeQuery {
    fn canvas_aspect(&self) -> f32;
    fn canvas_width(&self) -> u32;
    fn canvas_height(&self) -> u32;
    fn window_width(&self) -> u32;
    fn window_height(&self) -> u32;
}
