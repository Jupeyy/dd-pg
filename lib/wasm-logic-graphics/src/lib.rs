use std::sync::Arc;

use graphics::graphics::{GraphicsBackendHandle, GraphicsStreamHandle};
use graphics_backend::{backend::GraphicsBackend, checker::GraphicsContainersAPI, types::Graphics};
use graphics_backend_traits::traits::GraphicsBackendInterface;
use graphics_types::{command_buffer::AllCommands, rendering::GlVertex};
use wasm_runtime_types::{read_param, RawBytesEnv};
use wasmer::{imports, Function, FunctionEnv, FunctionEnvMut, Imports, Store};

pub struct WasmGraphicsLogicImpl {
    // this pointer should only be modified
    // before a wasm instance is called and
    // should be invalidated otherwise
    pub graphics_backend: GraphicsBackend,
    pub graphics_backend_handle: GraphicsBackendHandle<GraphicsBackend>,
    pub graphics_stream_handle: GraphicsStreamHandle<GraphicsBackend>,
    pub graphics_api: GraphicsContainersAPI,
}

impl WasmGraphicsLogicImpl {
    fn new(graphics: &mut Graphics, backend: GraphicsBackend, id_offset: u128) -> Self {
        Self {
            graphics_backend: backend,
            graphics_backend_handle: graphics.backend_handle.clone(),
            graphics_stream_handle: graphics.stream_handle.clone(),
            graphics_api: GraphicsContainersAPI::new(id_offset, graphics.backend_handle.clone()),
        }
    }

    fn run_cmds(
        &self,
        mut cmds: Vec<AllCommands>,
        vertices_param: Vec<GlVertex>,
        actually_run_cmds: bool,
    ) {
        let mut stream_data = self.graphics_stream_handle.stream_data.borrow_mut();
        let (vertices, vertices_count) = stream_data.vertices_and_count_mut();

        let must_flush_cmds = vertices.len() - *vertices_count < vertices_param.len();
        drop(stream_data);
        if must_flush_cmds {
            self.graphics_backend.run_cmds(
                &self.graphics_backend_handle.backend_cmds,
                &self.graphics_stream_handle.stream_data,
            );
        }

        let vertices_offset = self
            .graphics_stream_handle
            .stream_data
            .borrow()
            .vertices_count();

        let mut stream_data = self.graphics_stream_handle.stream_data.borrow_mut();
        let (vertices, vertices_count) = stream_data.vertices_and_count_mut();

        vertices[*vertices_count..*vertices_count + vertices_param.len()]
            .copy_from_slice(&vertices_param);
        *vertices_count += vertices_param.len();

        drop(stream_data);
        self.graphics_api.process_commands(
            &self.graphics_stream_handle,
            &mut cmds,
            vertices_offset,
        );

        self.graphics_backend_handle
            .backend_cmds
            .add_cmds(&mut cmds);

        if actually_run_cmds {
            self.graphics_backend.run_cmds(
                &self.graphics_backend_handle.backend_cmds,
                &self.graphics_stream_handle.stream_data,
            );
        }
    }
}

unsafe impl Send for WasmGraphicsLogicImpl {}
unsafe impl Sync for WasmGraphicsLogicImpl {}

pub struct WasmGraphicsLogic(pub Arc<WasmGraphicsLogicImpl>);

impl WasmGraphicsLogic {
    pub fn new(graphics: &mut Graphics, backend: GraphicsBackend, id_offset: u128) -> Self {
        Self(Arc::new(WasmGraphicsLogicImpl::new(
            graphics, backend, id_offset,
        )))
    }

    pub fn get_wasm_graphics_logic_imports(
        &self,
        store: &mut Store,
        raw_bytes_env: &FunctionEnv<Arc<RawBytesEnv>>,
    ) -> Imports {
        fn run_cmds(
            logic_clone: &Arc<WasmGraphicsLogicImpl>,
            mut env: FunctionEnvMut<Arc<RawBytesEnv>>,
        ) {
            let (data, mut store) = env.data_and_store_mut();
            let (mut param0, instance) = data.param_index_mut(0);
            let cmds = read_param(instance.as_ref().unwrap(), &mut store, &mut param0, 0);
            let (mut param1, instance) = data.param_index_mut(1);
            let vertices = read_param(instance.as_ref().unwrap(), &mut store, &mut param1, 1);
            let (mut param2, instance) = data.param_index_mut(2);
            let actually_run_cmds =
                read_param(instance.as_ref().unwrap(), &mut store, &mut param2, 2);

            logic_clone.run_cmds(cmds, vertices, actually_run_cmds)
        }

        let logic = self.0.clone();

        imports! {
            "env" => {
                "run_cmds" => Function::new_typed_with_env(store, raw_bytes_env, move |env: FunctionEnvMut<Arc<RawBytesEnv>>| run_cmds(&logic, env)),
            }
        }
    }
}
