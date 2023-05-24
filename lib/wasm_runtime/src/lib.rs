mod relaxed_atomic_optional_ptr;

use std::sync::Arc;

use graphics::graphics::Graphics;
use graphics_traits::GraphicsStreamHandler;
use graphics_types::{
    rendering::{GL_SVertex, State},
    types::DrawModes,
};
use relaxed_atomic_optional_ptr::RelaxedAtomicPtrOption;
use wasmer::{
    imports, CompilerConfig, Cranelift, Function, FunctionEnv, FunctionEnvMut, Instance, Module,
    Store, TypedFunction,
};

pub struct WasmManagerLogic {
    // this pointer should only be modified
    // before a wasm instance is called and
    // should be invalidated otherwise
    // TODO: force null check somehow
    graphics: RelaxedAtomicPtrOption<Graphics>,
}

impl WasmManagerLogic {
    fn flush_vertices(
        &self,
        vertices: &Vec<GL_SVertex>,
        state: &State,
        vertices_offset: usize,
        draw_mode: DrawModes,
    ) {
        let graphics = self.graphics.load();
        if let Some(graphics) = graphics {
            let (vertices, vertices_count) = graphics
                .backend_handle
                .backend_buffer_mut()
                .vertices_and_count_mut();
            println!("Hello, world!");
        }
    }
}

unsafe impl Send for WasmManagerLogic {}
unsafe impl Sync for WasmManagerLogic {}

/**
 * Creates a WASM instances, automatically uses and fills the cache
 * Note: Please never provide multi-threading support, it doesn't fit our design
 */
pub struct WasmManager {
    store: Store,
    instance: Instance,

    logic: Arc<WasmManagerLogic>,
}

impl WasmManager {
    pub fn new(wasm_bytes: &[u8]) -> anyhow::Result<Self> {
        let compiler = Cranelift::new();
        //compiler.opt_level(wasmer::CraneliftOptLevel::None);
        //compiler.enable_verifier();
        let mut store: Store = Store::new(compiler);
        // We then use our store and Wasm bytes to compile a `Module`.
        // A `Module` is a compiled WebAssembly module that isn't ready to execute yet.
        let module = Module::new(&store, wasm_bytes)?;

        let logic = Arc::new(WasmManagerLogic {
            graphics: RelaxedAtomicPtrOption::new(std::ptr::null_mut()),
        });

        let logic_clone = logic.clone();

        #[derive(Default, Clone)]
        struct RawBytesEnv {
            raw_bytes: Vec<u8>,
            raw_bytes2: Vec<u8>,
            raw_bytes3: Vec<u8>,
            raw_bytes4: Vec<u8>,
        }

        let println_env = FunctionEnv::new(&mut store, RawBytesEnv::default());

        fn raw_bytes_add_u64_impl(bytes: &mut Vec<u8>, byte_stream: u64, byte_count: u8) {
            // put bytes into our raw byte array
            assert!(byte_count as usize <= std::mem::size_of::<u64>(), "used byte count that is bigger than the size of u64, this must be a bug in the wasm module!");
            // some sanitizing
            assert!(
                (bytes.len() + byte_count as usize) < 1024 * 1024 * 1024,
                "using more than 1 GByte of memory is currently not allowed, please make sure the wasm module does not create such huge allocations."
            );
            let mut bytes_stream: [u8; std::mem::size_of::<u64>()] =
                [0; std::mem::size_of::<u64>()];
            bytes_stream.copy_from_slice(&byte_stream.to_le_bytes());
            bytes.extend_from_slice(bytes_stream.split_at(byte_count as usize).0);
        }

        fn raw_bytes_add_u64(
            mut env: FunctionEnvMut<RawBytesEnv>,
            byte_stream: u64,
            byte_count: u8,
        ) {
            raw_bytes_add_u64_impl(&mut env.data_mut().raw_bytes, byte_stream, byte_count)
        }

        fn raw_bytes_add_u64_2(
            mut env: FunctionEnvMut<RawBytesEnv>,
            byte_stream: u64,
            byte_count: u8,
        ) {
            raw_bytes_add_u64_impl(&mut env.data_mut().raw_bytes2, byte_stream, byte_count)
        }

        fn raw_bytes_add_u64_3(
            mut env: FunctionEnvMut<RawBytesEnv>,
            byte_stream: u64,
            byte_count: u8,
        ) {
            raw_bytes_add_u64_impl(&mut env.data_mut().raw_bytes3, byte_stream, byte_count)
        }

        fn raw_bytes_add_u64_4(
            mut env: FunctionEnvMut<RawBytesEnv>,
            byte_stream: u64,
            byte_count: u8,
        ) {
            raw_bytes_add_u64_impl(&mut env.data_mut().raw_bytes4, byte_stream, byte_count)
        }

        fn println(mut env: FunctionEnvMut<RawBytesEnv>) {
            let mut text: Vec<u8> = Default::default();
            std::mem::swap(&mut text, &mut env.data_mut().raw_bytes);
            let text_str = String::from_utf8(text);
            if let Ok(print_str) = text_str {
                println!("{}", print_str);
            }
        }

        fn flush_vertices(
            logic_clone: &Arc<WasmManagerLogic>,
            mut env: FunctionEnvMut<RawBytesEnv>,
            vertices_offset: u64,
        ) {
            let data = &mut env.data_mut();
            logic_clone.flush_vertices(
                &bincode::decode_from_slice(data.raw_bytes.as_slice(), bincode::config::standard())
                    .unwrap()
                    .0,
                &bincode::decode_from_slice(
                    data.raw_bytes2.as_slice(),
                    bincode::config::standard(),
                )
                .unwrap()
                .0,
                vertices_offset as usize,
                bincode::decode_from_slice(data.raw_bytes3.as_slice(), bincode::config::standard())
                    .unwrap()
                    .0,
            )
        }

        // We then create an import object so that the `Module`'s imports can be satisfied.
        let import_object = imports! {
            "env" => {
                "host_raw_bytes_add_u64" => Function::new_typed_with_env(&mut store, &println_env.clone(), raw_bytes_add_u64),
                "host_raw_bytes_add_u64_2" => Function::new_typed_with_env(&mut store, &println_env.clone(), raw_bytes_add_u64_2),
                "host_raw_bytes_add_u64_3" => Function::new_typed_with_env(&mut store, &println_env.clone(), raw_bytes_add_u64_3),
                "host_raw_bytes_add_u64_4" => Function::new_typed_with_env(&mut store, &println_env.clone(), raw_bytes_add_u64_4),
                "host_println" => Function::new_typed_with_env(&mut store, &println_env, println),
                "flush_vertices" => Function::new_typed_with_env(&mut store, &println_env, move |env: FunctionEnvMut<RawBytesEnv>, vertices_offset: u64| flush_vertices(&logic_clone, env, vertices_offset)),
            }
        };

        // We then use the `Module` and the import object to create an `Instance`.
        //
        // An `Instance` is a compiled WebAssembly module that has been set up
        // and is ready to execute.
        let instance = Instance::new(&mut store, &module, &import_object)?;

        Ok(Self {
            store: store,
            instance: instance,
            logic: logic,
        })
    }

    pub fn run(&mut self, graphics: &mut Graphics) -> anyhow::Result<()> {
        // We get the `TypedFunction` with no parameters and no results from the instance.
        //
        // Recall that the Wasm module exported a function named "run", this is getting
        // that exported function from the `Instance`.
        let run_func: TypedFunction<(), ()> = self
            .instance
            .exports
            .get_typed_function(&mut self.store, "api_run")?;

        // Finally, we call our exported Wasm function which will call our "say_hello"
        // function and return.
        self.logic.graphics.store(graphics);
        run_func.call(&mut self.store)?;
        self.logic.graphics.store(std::ptr::null_mut());
        Ok(())
    }
}
