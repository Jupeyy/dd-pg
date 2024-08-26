use std::{rc::Rc, sync::Arc};

use graphics::{
    graphics::graphics::Graphics,
    handles::{
        backend::backend::GraphicsBackendHandle, canvas::canvas::GraphicsCanvasHandle,
        stream::stream::GraphicsStreamHandle,
    },
};
use graphics_backend::{backend::GraphicsBackend, checker::GraphicsContainersAPI};
use graphics_backend_traits::traits::GraphicsBackendInterface;
use graphics_types::{commands::AllCommands, rendering::GlVertex};
use hiarc::Hiarc;
use wasm_runtime_types::{read_param, RawBytesEnv};
use wasmer::{imports, AsStoreRef, Function, FunctionEnv, FunctionEnvMut, Imports, Store};

#[derive(Debug, Hiarc)]
pub struct WasmGraphicsLogicImpl {
    pub graphics_backend: Rc<GraphicsBackend>,
    pub graphics_backend_handle: GraphicsBackendHandle,
    pub graphics_stream_handle: GraphicsStreamHandle,
    pub graphics_canvas_handle: GraphicsCanvasHandle,
    pub graphics_api: GraphicsContainersAPI,
}

impl WasmGraphicsLogicImpl {
    fn new(graphics: &Graphics, backend: Rc<GraphicsBackend>, id_offset: u128) -> Self {
        Self {
            graphics_backend: backend,
            graphics_backend_handle: graphics.backend_handle.clone(),
            graphics_stream_handle: graphics.stream_handle.clone(),
            graphics_canvas_handle: graphics.canvas_handle.clone(),
            graphics_api: GraphicsContainersAPI::new(id_offset, graphics.backend_handle.clone()),
        }
    }

    fn run_cmds(
        &self,
        mut cmds: Vec<AllCommands>,
        vertices_param: Vec<GlVertex>,
        uniform_instances: Vec<Vec<u8>>,
        actually_run_cmds: bool,
    ) {
        let stream_data = self.graphics_stream_handle.stream_data();
        let (vertices_len, vertices_count) = stream_data.max_vertices_len_and_cur_count();

        let must_flush_cmds = (vertices_len - vertices_count < vertices_param.len())
            || stream_data.uniform_is_full(uniform_instances.len());

        if must_flush_cmds {
            self.graphics_backend.run_cmds(
                &self.graphics_backend_handle.backend_cmds,
                self.graphics_stream_handle.stream_data(),
            );
        }

        let vertices_offset = self.graphics_stream_handle.stream_data().vertices_count();

        let stream_data = self.graphics_stream_handle.stream_data();
        stream_data.add_vertices(&vertices_param);

        let uniform_offset = stream_data.deserialize_uniform_instances_from_vec(uniform_instances);

        self.graphics_api.process_commands(
            &self.graphics_stream_handle,
            &self.graphics_canvas_handle,
            &mut cmds,
            vertices_offset,
            uniform_offset,
        );

        self.graphics_backend_handle
            .backend_cmds
            .add_cmds(&mut cmds);

        if actually_run_cmds {
            self.graphics_backend.run_cmds(
                &self.graphics_backend_handle.backend_cmds,
                self.graphics_stream_handle.stream_data(),
            );
        }
    }
}

unsafe impl Send for WasmGraphicsLogicImpl {}
unsafe impl Sync for WasmGraphicsLogicImpl {}

pub struct WasmGraphicsLogic(pub Arc<WasmGraphicsLogicImpl>);

impl WasmGraphicsLogic {
    pub fn new(graphics: &Graphics, backend: Rc<GraphicsBackend>, id_offset: u128) -> Self {
        Self(Arc::new(WasmGraphicsLogicImpl::new(
            graphics, backend, id_offset,
        )))
    }

    pub fn get_wasm_logic_imports(
        &self,
        store: &mut Store,
        raw_bytes_env: &FunctionEnv<Arc<RawBytesEnv>>,
    ) -> Imports {
        fn run_cmds(
            logic_clone: &Arc<WasmGraphicsLogicImpl>,
            mut env: FunctionEnvMut<Arc<RawBytesEnv>>,
        ) {
            let (data, store) = env.data_and_store_mut();
            let (mut param0, instance) = data.param_index_mut(0);
            let cmds = read_param(
                instance.as_ref().unwrap(),
                &store.as_store_ref(),
                &mut param0,
                0,
            );
            let (mut param1, instance) = data.param_index_mut(1);
            let vertices = read_param(
                instance.as_ref().unwrap(),
                &store.as_store_ref(),
                &mut param1,
                1,
            );
            let (mut param2, instance) = data.param_index_mut(2);
            let uniform_instances = read_param(
                instance.as_ref().unwrap(),
                &store.as_store_ref(),
                &mut param2,
                2,
            );
            let (mut param2, instance) = data.param_index_mut(3);
            let actually_run_cmds = read_param(
                instance.as_ref().unwrap(),
                &store.as_store_ref(),
                &mut param2,
                3,
            );

            logic_clone.run_cmds(cmds, vertices, uniform_instances, actually_run_cmds)
        }

        let logic = self.0.clone();

        imports! {
            "env" => {
                "run_cmds" => Function::new_typed_with_env(store, raw_bytes_env, move |env: FunctionEnvMut<Arc<RawBytesEnv>>| run_cmds(&logic, env)),
            }
        }
    }
}
