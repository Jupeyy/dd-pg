use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::Arc,
};

use base_io::{io::Io, io_batcher::IoBatcherTask};
use wasm_runtime_types::{read_param, write_result, RawBytesEnv};
use wasmer::{imports, Function, FunctionEnv, FunctionEnvMut, Imports, Store};

pub struct WasmFileSystemLogicImpl {
    pub io: Io,
    tasks: RefCell<HashMap<u64, IoBatcherTask<Vec<u8>>>>,
    dir_tasks: RefCell<HashMap<u64, IoBatcherTask<HashMap<PathBuf, Vec<u8>>>>>,
    entries_tasks: RefCell<HashMap<u64, IoBatcherTask<HashSet<String>>>>,
}

impl WasmFileSystemLogicImpl {
    fn new(io: Io) -> Self {
        Self {
            io,
            tasks: Default::default(),
            dir_tasks: Default::default(),
            entries_tasks: Default::default(),
        }
    }

    fn read_file(&self, file_id: u64, file_path: &Path) -> Option<Result<Vec<u8>, String>> {
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
                let file_path_str = file_path.to_path_buf();
                let task = self
                    .io
                    .io_batcher
                    .spawn(async move { Ok(fs.read_file(&file_path_str).await?) });
                tasks.insert(file_id, task);
                None
            }
        }
    }

    fn files_in_dir_recursive(
        &self,
        file_id: u64,
        path: &Path,
    ) -> Option<Result<HashMap<PathBuf, Vec<u8>>, String>> {
        let mut tasks = self.dir_tasks.borrow_mut();
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
                let path_str = path.to_path_buf();
                let task = self
                    .io
                    .io_batcher
                    .spawn(async move { Ok(fs.files_in_dir_recursive(&path_str).await?) });
                tasks.insert(file_id, task);
                None
            }
        }
    }

    fn entries_in_dir(&self, file_id: u64, path: &Path) -> Option<Result<HashSet<String>, String>> {
        let mut tasks = self.entries_tasks.borrow_mut();
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
                let path_str = path.to_path_buf();
                let task = self
                    .io
                    .io_batcher
                    .spawn(async move { Ok(fs.entries_in_dir(&path_str).await?) });
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
    pub fn new(io: Io) -> Self {
        Self(Arc::new(WasmFileSystemLogicImpl::new(io)))
    }

    pub fn get_wasm_logic_imports(
        &self,
        store: &mut Store,
        raw_bytes_env: &FunctionEnv<Arc<RawBytesEnv>>,
    ) -> Imports {
        fn read_file(
            logic_clone: &Arc<WasmFileSystemLogicImpl>,
            mut env: FunctionEnvMut<Arc<RawBytesEnv>>,
        ) {
            let (data, mut store) = env.data_and_store_mut();
            let (mut param0, instance) = data.param_index_mut(0);
            let file_path: PathBuf =
                read_param(instance.as_ref().unwrap(), &mut store, &mut param0, 0);
            let file_id: u64 = read_param(instance.as_ref().unwrap(), &mut store, &mut param0, 1);

            let file = logic_clone.read_file(file_id, &file_path);
            write_result(instance.as_ref().unwrap(), &mut store, &file);
        }

        fn files_in_dir_recursive(
            logic_clone: &Arc<WasmFileSystemLogicImpl>,
            mut env: FunctionEnvMut<Arc<RawBytesEnv>>,
        ) {
            let (data, mut store) = env.data_and_store_mut();
            let (mut param0, instance) = data.param_index_mut(0);
            let dir_path: PathBuf =
                read_param(instance.as_ref().unwrap(), &mut store, &mut param0, 0);
            let file_id: u64 = read_param(instance.as_ref().unwrap(), &mut store, &mut param0, 1);

            let file = logic_clone.files_in_dir_recursive(file_id, &dir_path);
            write_result(instance.as_ref().unwrap(), &mut store, &file);
        }

        fn entries_in_dir(
            logic_clone: &Arc<WasmFileSystemLogicImpl>,
            mut env: FunctionEnvMut<Arc<RawBytesEnv>>,
        ) {
            let (data, mut store) = env.data_and_store_mut();
            let (mut param0, instance) = data.param_index_mut(0);
            let dir_path: PathBuf =
                read_param(instance.as_ref().unwrap(), &mut store, &mut param0, 0);
            let file_id: u64 = read_param(instance.as_ref().unwrap(), &mut store, &mut param0, 1);

            let file = logic_clone.entries_in_dir(file_id, &dir_path);
            write_result(instance.as_ref().unwrap(), &mut store, &file);
        }

        let logic = self.0.clone();
        let logic2 = self.0.clone();
        let logic3 = self.0.clone();

        imports! {
            "env" => {
                "api_read_file" => Function::new_typed_with_env(store, raw_bytes_env, move |env: FunctionEnvMut<Arc<RawBytesEnv>>| read_file(&logic, env)),
                "api_files_in_dir_recursive" => Function::new_typed_with_env(store, raw_bytes_env, move |env: FunctionEnvMut<Arc<RawBytesEnv>>| files_in_dir_recursive(&logic2, env)),
                "api_entries_in_dir" => Function::new_typed_with_env(store, raw_bytes_env, move |env: FunctionEnvMut<Arc<RawBytesEnv>>| entries_in_dir(&logic3, env)),
            }
        }
    }
}
