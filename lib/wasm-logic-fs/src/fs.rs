use std::{cell::RefCell, collections::HashMap, sync::Arc};

use base_io::{io::IO, io_batcher::TokIOBatcherTask};
use wasm_runtime_types::{read_param, write_result, RawBytesEnv};
use wasmer::{imports, Function, FunctionEnv, FunctionEnvMut, Imports, Store};

pub struct WasmFileSystemLogicImpl {
    pub io: IO,
    tasks: RefCell<HashMap<u64, TokIOBatcherTask<Vec<u8>>>>,
}

impl WasmFileSystemLogicImpl {
    fn new(io: IO) -> Self {
        Self {
            io,
            tasks: Default::default(),
        }
    }

    fn open_file(&self, file_id: u64, file_path: &str) -> Option<Result<Vec<u8>, String>> {
        let mut tasks = self.tasks.borrow_mut();
        match tasks.get(&file_id) {
            Some(task) => {
                if task.is_finished() {
                    let task = tasks.remove(&file_id).unwrap();
                    Some(task.get_storage().map_err(|err| err.to_string()))
                } else {
                    None
                }
            }
            None => {
                let fs = self.io.fs.clone();
                let file_path_str = file_path.to_string();
                let task = self
                    .io
                    .io_batcher
                    .spawn(async move { Ok(fs.open_file(&file_path_str).await?) });
                tasks.insert(file_id, task);
                None
            }
        }
    }
}

unsafe impl Send for WasmFileSystemLogicImpl {}
unsafe impl Sync for WasmFileSystemLogicImpl {}

pub struct WasmFileSystemLogic(pub Arc<WasmFileSystemLogicImpl>);

impl WasmFileSystemLogic {
    pub fn new(io: IO) -> Self {
        Self(Arc::new(WasmFileSystemLogicImpl::new(io)))
    }

    pub fn get_wasm_graphics_logic_imports(
        &self,
        store: &mut Store,
        raw_bytes_env: &FunctionEnv<Arc<RawBytesEnv>>,
    ) -> Imports {
        fn open_file(
            logic_clone: &Arc<WasmFileSystemLogicImpl>,
            mut env: FunctionEnvMut<Arc<RawBytesEnv>>,
        ) {
            let (data, mut store) = env.data_and_store_mut();
            let (mut param0, instance) = data.param_index_mut(0);
            let file_path: String =
                read_param(instance.as_ref().unwrap(), &mut store, &mut param0, 0);
            let file_id: u64 = read_param(instance.as_ref().unwrap(), &mut store, &mut param0, 1);

            let file = logic_clone.open_file(file_id, &file_path);
            write_result(instance.as_ref().unwrap(), &mut store, &file);
        }

        let logic = self.0.clone();

        imports! {
            "env" => {
                "api_open_file" => Function::new_typed_with_env(store, raw_bytes_env, move |env: FunctionEnvMut<Arc<RawBytesEnv>>| open_file(&logic, env)),
            }
        }
    }
}
