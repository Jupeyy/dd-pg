use graphics_base::streaming::DrawQuads;
use graphics_traits::{GraphicsBachendBufferInterface, GraphicsStreamHandler};
use graphics_types::{
    rendering::{SVertex, State},
    types::DrawModes,
};

use crate::upload_bytes;

extern "C" {
    fn flush_vertices(vertices_offset: u64);
}

struct BackendBuffer {
    vertices: Vec<SVertex>,
    vertices_count: usize,
}

impl BackendBuffer {
    pub fn new() -> Self {
        let mut vertices: Vec<_> = Vec::new();
        vertices.resize(256, Default::default());
        Self {
            vertices: vertices,
            vertices_count: 0,
        }
    }
}

impl GraphicsBachendBufferInterface for BackendBuffer {
    fn vertices_mut(&mut self) -> &mut [SVertex] {
        &mut self.vertices[..]
    }

    fn vertices_count(&self) -> usize {
        self.vertices_count
    }

    fn vertices_count_mut(&mut self) -> &mut usize {
        &mut self.vertices_count
    }

    fn vertices_and_count_mut(&mut self) -> (&mut [SVertex], &mut usize) {
        (&mut self.vertices, &mut self.vertices_count)
    }
}

struct BackendHandle {
    buffer: BackendBuffer,
}

impl BackendHandle {
    pub fn new() -> Self {
        Self {
            buffer: BackendBuffer::new(),
        }
    }
}

impl GraphicsStreamHandler for BackendHandle {
    fn backend_buffer_mut(&mut self) -> &mut dyn GraphicsBachendBufferInterface {
        &mut self.buffer
    }

    fn flush_vertices(&mut self, state: &State, vertices_offset: usize, draw_mode: DrawModes) {
        upload_bytes(
            0,
            bincode::encode_to_vec(
                self.buffer.vertices[vertices_offset..self.buffer.vertices_count].to_vec(),
                bincode::config::standard(),
            )
            .unwrap()
            .as_slice(),
        ); // TODO: vertices
        upload_bytes(
            1,
            bincode::encode_to_vec(state, bincode::config::standard())
                .unwrap()
                .as_slice(),
        ); // state
        upload_bytes(
            2,
            bincode::encode_to_vec(draw_mode, bincode::config::standard())
                .unwrap()
                .as_slice(),
        ); // draw_mode

        unsafe { flush_vertices(vertices_offset as u64) }

        self.buffer.vertices_count = 0;
    }

    fn run_backend_buffer(&mut self) {
        todo!("should this really ever be supported")
    }

    fn add_cmd(&mut self, _cmd: graphics_types::command_buffer::AllCommands) {
        todo!("should this really ever be supported")
    }
}

/**
 * The API to use graphics similar to the host graphics class
 */
pub struct Graphics {
    backend_handle: BackendHandle,
}

impl Graphics {
    pub fn new() -> Self {
        Self {
            backend_handle: BackendHandle::new(),
        }
    }

    pub fn quads_begin(&mut self) -> DrawQuads {
        let vertices_offset = self.backend_handle.buffer.vertices.len();
        DrawQuads::new(&mut self.backend_handle, vertices_offset)
    }
}
