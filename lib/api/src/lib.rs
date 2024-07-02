#![allow(clippy::all)]

use std::{
    cell::{Cell, RefCell},
    ptr::addr_of_mut,
    rc::Rc,
    sync::Arc,
};

use ::graphics::graphics::graphics::Graphics;
use ::sound::sound::SoundManager;
use base_fs::filesys::FileSystem;
use base_http::http::HttpClient;
use base_io::io::Io;
use graphics::graphics::GraphicsBackend;
use graphics_base_traits::traits::{
    GraphicsStreamVertices, GraphicsStreamedData, GraphicsStreamedUniformData,
    GraphicsStreamedUniformRawData,
};
use graphics_types::{
    commands::{
        StreamDataMax, GRAPHICS_DEFAULT_UNIFORM_SIZE, GRAPHICS_MAX_UNIFORM_RENDER_COUNT,
        GRAPHICS_UNIFORM_INSTANCE_COUNT,
    },
    types::WindowProps,
};
use pool::mt_datatypes::PoolVec;
use serde::{de::DeserializeOwned, Serialize};
use sound::sound_backend::SoundBackend;

pub mod base_fs;
pub mod base_http;
pub mod graphics;
pub mod sound;

extern "C" {
    fn host_println();
}

pub static mut GRAPHICS_BACKEND: once_cell::unsync::Lazy<Rc<GraphicsBackend>> =
    once_cell::unsync::Lazy::new(|| {
        Rc::new(GraphicsBackend {
            actual_run_cmds: Cell::new(true),
            sync_points: Default::default(),
        })
    });

pub static mut GRAPHICS: once_cell::unsync::Lazy<RefCell<Graphics>> =
    once_cell::unsync::Lazy::new(|| {
        let mut uniform_buffers = PoolVec::new_without_pool();
        uniform_buffers.resize_with(GRAPHICS_UNIFORM_INSTANCE_COUNT, || {
            GraphicsStreamedUniformData::new(GraphicsStreamedUniformRawData::Vector(
                vec![0; GRAPHICS_MAX_UNIFORM_RENDER_COUNT * GRAPHICS_DEFAULT_UNIFORM_SIZE],
            ))
        });
        RefCell::new(Graphics::new(
            unsafe { GRAPHICS_BACKEND.clone() },
            GraphicsStreamedData::new(
                GraphicsStreamVertices::Vec(vec![
                    Default::default();
                    StreamDataMax::MaxVertices as usize
                ]),
                uniform_buffers,
            ),
            WindowProps {
                canvas_width: 800.0,
                canvas_height: 600.0,
                window_width: 800,
                window_height: 600,
            },
        ))
    });

pub static mut SOUND: once_cell::unsync::Lazy<RefCell<SoundManager>> =
    once_cell::unsync::Lazy::new(|| {
        let mut uniform_buffers = PoolVec::new_without_pool();
        uniform_buffers.resize_with(GRAPHICS_UNIFORM_INSTANCE_COUNT, || {
            GraphicsStreamedUniformData::new(GraphicsStreamedUniformRawData::Vector(
                vec![0; GRAPHICS_MAX_UNIFORM_RENDER_COUNT * GRAPHICS_DEFAULT_UNIFORM_SIZE],
            ))
        });
        RefCell::new(SoundManager::new(Box::new(SoundBackend {})).unwrap())
    });

pub static mut IO: once_cell::unsync::Lazy<RefCell<Io>> = once_cell::unsync::Lazy::new(|| {
    RefCell::new(Io::new(
        |_| Arc::new(FileSystem::new()),
        Arc::new(HttpClient::new()),
    ))
});

pub static RUNTIME_THREAD_POOL: once_cell::sync::Lazy<Arc<rayon::ThreadPool>> =
    once_cell::sync::Lazy::new(|| {
        Arc::new(
            rayon::ThreadPoolBuilder::default()
                .num_threads(1)
                .use_current_thread()
                .build()
                .unwrap(),
        )
    });

// for system
#[no_mangle]
fn sys_print(log_str: &str) {
    println(log_str);
}

pub struct Logger {}

impl log::Log for Logger {
    fn enabled(&self, _metadata: &log::Metadata) -> bool {
        true
    }

    fn log(&self, record: &log::Record) {
        println(&format!("wasm log msg: {:?}", record));
    }

    fn flush(&self) {}
}

#[no_mangle]
pub fn api_setup() {
    std::panic::set_hook(Box::new(|panic_info| {
        let panic_text = format!("wasm module {}", panic_info.to_string());
        println(panic_text);
    }));
    log::set_boxed_logger(Box::new(Logger {})).unwrap();
    log::set_max_level(log::LevelFilter::Info);
}

// shared memory
static mut RES: Vec<u8> = Vec::new();
#[no_mangle]
pub static mut RESULT_PTR: i32 = 0;
#[no_mangle]
pub static mut RESULT_SIZE: i32 = 0;

static mut PARAMS: once_cell::unsync::Lazy<[Vec<u8>; 10]> =
    once_cell::unsync::Lazy::new(|| Default::default());

#[no_mangle]
pub static mut PARAM0_PTR: i32 = 0;
#[no_mangle]
pub static mut PARAM0_SIZE: i32 = 0;
#[no_mangle]
pub static mut PARAM1_PTR: i32 = 0;
#[no_mangle]
pub static mut PARAM1_SIZE: i32 = 0;
#[no_mangle]
pub static mut PARAM2_PTR: i32 = 0;
#[no_mangle]
pub static mut PARAM2_SIZE: i32 = 0;
#[no_mangle]
pub static mut PARAM3_PTR: i32 = 0;
#[no_mangle]
pub static mut PARAM3_SIZE: i32 = 0;
#[no_mangle]
pub static mut PARAM4_PTR: i32 = 0;
#[no_mangle]
pub static mut PARAM4_SIZE: i32 = 0;
#[no_mangle]
pub static mut PARAM5_PTR: i32 = 0;
#[no_mangle]
pub static mut PARAM5_SIZE: i32 = 0;
#[no_mangle]
pub static mut PARAM6_PTR: i32 = 0;
#[no_mangle]
pub static mut PARAM6_SIZE: i32 = 0;
#[no_mangle]
pub static mut PARAM7_PTR: i32 = 0;
#[no_mangle]
pub static mut PARAM7_SIZE: i32 = 0;
#[no_mangle]
pub static mut PARAM8_PTR: i32 = 0;
#[no_mangle]
pub static mut PARAM8_SIZE: i32 = 0;
#[no_mangle]
pub static mut PARAM9_PTR: i32 = 0;
#[no_mangle]
pub static mut PARAM9_SIZE: i32 = 0;

fn set_param_params(index: usize) {
    unsafe {
        let (ptr, size) = match index {
            0 => (addr_of_mut!(PARAM0_PTR), addr_of_mut!(PARAM0_SIZE)),
            1 => (addr_of_mut!(PARAM1_PTR), addr_of_mut!(PARAM1_SIZE)),
            2 => (addr_of_mut!(PARAM2_PTR), addr_of_mut!(PARAM2_SIZE)),
            3 => (addr_of_mut!(PARAM3_PTR), addr_of_mut!(PARAM3_SIZE)),
            4 => (addr_of_mut!(PARAM4_PTR), addr_of_mut!(PARAM4_SIZE)),
            5 => (addr_of_mut!(PARAM5_PTR), addr_of_mut!(PARAM5_SIZE)),
            6 => (addr_of_mut!(PARAM6_PTR), addr_of_mut!(PARAM6_SIZE)),
            7 => (addr_of_mut!(PARAM7_PTR), addr_of_mut!(PARAM7_SIZE)),
            8 => (addr_of_mut!(PARAM8_PTR), addr_of_mut!(PARAM8_SIZE)),
            9 => (addr_of_mut!(PARAM9_PTR), addr_of_mut!(PARAM9_SIZE)),
            _ => panic!("unsupported param index"),
        };
        *ptr = PARAMS[index].as_ptr() as i32;
        *size = PARAMS[index].len() as i32;
    }
}

pub fn upload_param<F: Serialize>(index: usize, data: F) {
    unsafe {
        PARAMS[index].clear();
    }

    bincode::serde::encode_into_std_write::<F, _, Vec<_>>(
        data,
        unsafe { &mut PARAMS[index] },
        bincode::config::standard().with_fixed_int_encoding(),
    )
    .unwrap();

    set_param_params(index);
}

pub fn println<F: Serialize + std::string::ToString>(text: F) {
    upload_param(0, text.to_string());
    unsafe { host_println() };
}

pub fn upload_return_val<F: Serialize>(res: F) {
    unsafe {
        RES.clear();
    }

    bincode::serde::encode_into_std_write::<F, _, _>(
        res,
        unsafe { &mut *addr_of_mut!(RES) },
        bincode::config::standard().with_fixed_int_encoding(),
    )
    .unwrap();

    unsafe {
        RESULT_PTR = RES.as_ptr() as i32;
        RESULT_SIZE = RES.len() as i32;
    }
}

fn read_param_from_host_checked<F: DeserializeOwned>(
    index: u32,
) -> Result<F, bincode::error::DecodeError> {
    unsafe {
        bincode::serde::decode_from_slice(
            PARAMS[index as usize].as_slice(),
            bincode::config::standard().with_fixed_int_encoding(),
        )
        .map(|opt| opt.0)
    }
}

pub fn read_param_from_host<F: DeserializeOwned>(index: u32) -> F {
    read_param_from_host_checked(index).unwrap()
}

pub fn read_param_from_host_ex<F: DeserializeOwned>(
    index: u32,
    ty_name: &str,
    caller_name: &str,
) -> F {
    unsafe {
        read_param_from_host_checked::<F>(index).unwrap_or_else(|e| {
            panic!(
                "error decoding type \"{}\" ({:?}): {} called by {}",
                ty_name, PARAMS[index as usize], e, caller_name
            )
        })
    }
}

fn read_result_from_host_checked<F: DeserializeOwned>() -> Result<F, bincode::error::DecodeError> {
    unsafe {
        bincode::serde::decode_from_slice(
            RES.as_slice(),
            bincode::config::standard().with_fixed_int_encoding(),
        )
        .map(|opt| opt.0)
    }
}

pub fn read_result_from_host<F: DeserializeOwned>() -> F {
    read_result_from_host_checked().unwrap()
}

#[no_mangle]
pub fn prepare_param(index: u32, expected_size: u32) {
    unsafe { PARAMS[index as usize].resize(expected_size as usize, Default::default()) };
    set_param_params(index as usize);
}

#[no_mangle]
pub fn prepare_result(expected_size: u32) {
    unsafe {
        RES.resize(expected_size as usize, Default::default());
        RESULT_PTR = RES.as_ptr() as i32;
        RESULT_SIZE = RES.len() as i32;
    }
}
