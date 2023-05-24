use graphics_types::{
    command_buffer::AllCommands,
    rendering::{SVertex, State},
    types::DrawModes,
};

pub trait GraphicsBachendBufferInterface {
    fn vertices_mut(&mut self) -> &mut [SVertex];
    fn vertices_count(&self) -> usize;
    fn vertices_count_mut(&mut self) -> &mut usize;
    fn vertices_and_count_mut(&mut self) -> (&mut [SVertex], &mut usize);
}

pub trait GraphicsStreamHandler {
    fn backend_buffer_mut(&mut self) -> &mut dyn GraphicsBachendBufferInterface;

    fn flush_vertices(&mut self, state: &State, vertices_offset: usize, draw_mode: DrawModes);

    fn run_backend_buffer(&mut self);

    fn add_cmd(&mut self, cmd: AllCommands);
}
