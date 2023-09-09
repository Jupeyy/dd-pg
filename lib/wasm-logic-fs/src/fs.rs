use std::sync::Arc;

use base::atomic_optional_ptr::AtomicPtrOption;
use base_fs::filesys::FileSystem;
use base_fs_traits::traits::FileSystemInterface;
use pollster::FutureExt;
use wasm_runtime_types::{read_param, write_result, RawBytesEnv};
use wasmer::{imports, Function, FunctionEnv, FunctionEnvMut, Imports, Store};

pub struct WasmFileSystemLogicImpl {
    // this pointer should only be modified
    // before a wasm instance is called and
    // should be invalidated otherwise
    pub fs: AtomicPtrOption<FileSystem>,
}

impl WasmFileSystemLogicImpl {
    fn new() -> Self {
        Self {
            fs: Default::default(),
        }
    }

    fn open_file(&self, file_path: &str) -> std::io::Result<Vec<u8>> {
        let fs = self.fs.load().unwrap();
        fs.open_file(file_path).block_on()
    }
}

unsafe impl Send for WasmFileSystemLogicImpl {}
unsafe impl Sync for WasmFileSystemLogicImpl {}

pub struct WasmFileSystemLogic(pub Arc<WasmFileSystemLogicImpl>);

impl WasmFileSystemLogic {
    pub fn new() -> Self {
        Self(Arc::new(WasmFileSystemLogicImpl::new()))
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
            write_result(
                instance.as_ref().unwrap(),
                &mut store,
                &logic_clone.open_file(&file_path).unwrap(),
            );
        }

        let logic = self.0.clone();

        imports! {
            "env" => {
                "api_open_file" => Function::new_typed_with_env(store, raw_bytes_env, move |env: FunctionEnvMut<Arc<RawBytesEnv>>| open_file(&logic, env)),
            }
        }
    }
}
