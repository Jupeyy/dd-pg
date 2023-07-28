use std::sync::Arc;

use base::atomic_optional_ptr::AtomicPtrOption;
use graphics::graphics::Graphics;
use graphics_traits::GraphicsStreamHandler;
use graphics_types::{
    command_buffer::StreamDataMax,
    rendering::{GlVertex, State},
    types::DrawModes,
};
use wasm_runtime_types::{read_param, RawBytesEnv};
use wasmer::{imports, Function, FunctionEnv, FunctionEnvMut, Imports, Store};

pub struct WasmGraphicsLogicImpl {
    // this pointer should only be modified
    // before a wasm instance is called and
    // should be invalidated otherwise
    pub graphics: AtomicPtrOption<Graphics>,
}

impl WasmGraphicsLogicImpl {
    fn new() -> Self {
        Self {
            graphics: Default::default(),
        }
    }

    fn flush_vertices(&self, vertices: &Vec<GlVertex>, state: &State, draw_mode: DrawModes) {
        let graphics = self.graphics.load();
        // first check if number of vertices is ok for the draw type
        let vert_per_primitive = match draw_mode {
            DrawModes::Quads => 4,
            DrawModes::Lines => 2,
            DrawModes::Triangles => 3,
        };
        assert!(
            vertices.len() % vert_per_primitive == 0,
            "wasm module implementation was invalid, used incorrect number of vertices."
        );
        assert!(
            vertices.len() <= StreamDataMax::MaxVertices as usize,
            "Please don't use more vertices than you should, this only hurts performance."
        );
        assert!(
            state.get_canvas_width() > 0.0 && state.get_canvas_height() > 0.0,
            "canvas had a width or height of <= 0, this is rarely intentional."
        );
        if let Some(graphics) = graphics {
            let (mut bk_vertices, mut bk_vertices_count) = graphics
                .backend_handle
                .backend_buffer_mut()
                .vertices_and_count_mut();

            // flush all vertices over and over until all are flushed
            let mut vert_count = vertices.len();
            let mut vert_offset = 0;
            while vert_count > 0 {
                let vertices_offset = *bk_vertices_count;
                let mut flush_count = (bk_vertices.len() - *bk_vertices_count).min(vert_count);
                flush_count = flush_count - (flush_count % vert_per_primitive);

                if flush_count > 0 {
                    bk_vertices[*bk_vertices_count..*bk_vertices_count + flush_count]
                        .copy_from_slice(&vertices[vert_offset..vert_offset + flush_count]);
                    *bk_vertices_count += flush_count;

                    graphics
                        .backend_handle
                        .flush_vertices(state, vertices_offset, draw_mode);
                    vert_offset += flush_count;
                    vert_count -= flush_count;
                } else {
                    graphics.backend_handle.run_backend_buffer();
                }
                (bk_vertices, bk_vertices_count) = graphics
                    .backend_handle
                    .backend_buffer_mut()
                    .vertices_and_count_mut();
            }
        }
    }
}

unsafe impl Send for WasmGraphicsLogicImpl {}
unsafe impl Sync for WasmGraphicsLogicImpl {}

pub struct WasmGraphicsLogic(pub Arc<WasmGraphicsLogicImpl>);

impl WasmGraphicsLogic {
    pub fn new() -> Self {
        Self(Arc::new(WasmGraphicsLogicImpl::new()))
    }

    pub fn get_wasm_graphics_logic_imports(
        &self,
        store: &mut Store,
        raw_bytes_env: &FunctionEnv<Arc<RawBytesEnv>>,
    ) -> Imports {
        fn flush_vertices(
            logic_clone: &Arc<WasmGraphicsLogicImpl>,
            mut env: FunctionEnvMut<Arc<RawBytesEnv>>,
        ) {
            let (data, mut store) = env.data_and_store_mut();
            let (mut param0, instance) = data.param_index_mut(0);
            let vertices = read_param(instance.as_ref().unwrap(), &mut store, &mut param0, 0);
            let (mut param1, instance) = data.param_index_mut(1);
            let state = read_param(instance.as_ref().unwrap(), &mut store, &mut param1, 1);
            let (mut param2, instance) = data.param_index_mut(2);
            let draw_mode = read_param(instance.as_ref().unwrap(), &mut store, &mut param2, 2);
            logic_clone.flush_vertices(&vertices, &state, draw_mode)
        }

        let logic = self.0.clone();

        imports! {
            "env" => {
                "flush_vertices" => Function::new_typed_with_env(store, raw_bytes_env, move |env: FunctionEnvMut<Arc<RawBytesEnv>>| flush_vertices(&logic, env)),
            }
        }
    }
}
