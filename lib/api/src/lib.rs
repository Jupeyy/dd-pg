use std::{cell::RefCell, rc::Rc};

use graphics::graphics::{BackendStreamData, Graphics, GraphicsBackend};
use graphics_types::types::WindowProps;

pub mod base_fs;
pub mod graphics;

extern "C" {
    fn host_println();
}

pub static mut GRAPHICS_BACKEND: once_cell::unsync::Lazy<GraphicsBackend> =
    once_cell::unsync::Lazy::new(|| GraphicsBackend {
        actual_run_cmds: Rc::new(RefCell::new(true)),
    });

pub static mut GRAPHICS: once_cell::unsync::Lazy<Graphics> = once_cell::unsync::Lazy::new(|| {
    Graphics::new(
        unsafe { GRAPHICS_BACKEND.clone() },
        Rc::new(RefCell::new(BackendStreamData::new())),
        WindowProps {
            canvas_width: 0.0,
            canvas_height: 0.0,
            window_width: 0,
            window_height: 0,
        },
    )
});

// for system
#[no_mangle]
fn sys_print(log_str: &str) {
    println(log_str);
}

#[no_mangle]
pub fn api_setup() {
    std::panic::set_hook(Box::new(|panic_info| {
        let panic_text = format!("wasm module {}", panic_info.to_string());
        println(panic_text);
    }));
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
            0 => (&mut PARAM0_PTR, &mut PARAM0_SIZE),
            1 => (&mut PARAM1_PTR, &mut PARAM1_SIZE),
            2 => (&mut PARAM2_PTR, &mut PARAM2_SIZE),
            3 => (&mut PARAM3_PTR, &mut PARAM3_SIZE),
            4 => (&mut PARAM4_PTR, &mut PARAM4_SIZE),
            5 => (&mut PARAM5_PTR, &mut PARAM5_SIZE),
            6 => (&mut PARAM6_PTR, &mut PARAM6_SIZE),
            7 => (&mut PARAM7_PTR, &mut PARAM7_SIZE),
            8 => (&mut PARAM8_PTR, &mut PARAM8_SIZE),
            9 => (&mut PARAM9_PTR, &mut PARAM9_SIZE),
            _ => panic!("unsupported param index"),
        };
        *ptr = PARAMS[index].as_ptr() as i32;
        *size = PARAMS[index].len() as i32;
    }
}

pub fn upload_param<F: bincode::Encode>(index: usize, data: F) {
    unsafe {
        PARAMS[index].clear();
    }

    bincode::encode_into_std_write::<F, _, _>(
        data,
        &mut std::io::BufWriter::<&mut Vec<u8>>::new(unsafe { PARAMS[index].as_mut() }),
        bincode::config::standard().with_fixed_int_encoding(),
    )
    .unwrap();

    set_param_params(index);
}

pub fn println<F: bincode::Encode + std::string::ToString>(text: F) {
    upload_param(0, text.to_string());
    unsafe { host_println() };
}

pub fn upload_return_val<F: bincode::Encode>(res: F) {
    unsafe {
        RES.clear();
    }

    bincode::encode_into_std_write::<F, _, _>(
        res,
        &mut std::io::BufWriter::<&mut Vec<u8>>::new(unsafe { RES.as_mut() }),
        bincode::config::standard().with_fixed_int_encoding(),
    )
    .unwrap();

    unsafe {
        RESULT_PTR = RES.as_ptr() as i32;
        RESULT_SIZE = RES.len() as i32;
    }
}

fn read_param_from_host_checked<F: bincode::Decode>(
    index: u32,
) -> Result<F, bincode::error::DecodeError> {
    unsafe {
        bincode::decode_from_slice(
            PARAMS[index as usize].as_slice(),
            bincode::config::standard().with_fixed_int_encoding(),
        )
        .map(|opt| opt.0)
    }
}

pub fn read_param_from_host<F: bincode::Decode>(index: u32) -> F {
    read_param_from_host_checked(index).unwrap()
}

pub fn read_param_from_host_ex<F: bincode::Decode>(
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

fn read_result_from_host_checked<F: bincode::Decode>() -> Result<F, bincode::error::DecodeError> {
    unsafe {
        bincode::decode_from_slice(
            RES.as_slice(),
            bincode::config::standard().with_fixed_int_encoding(),
        )
        .map(|opt| opt.0)
    }
}

pub fn read_result_from_host<F: bincode::Decode>() -> F {
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
