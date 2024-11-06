#[cfg(target_arch = "wasm32")]
extern "Rust" {
    fn sys_print(str: &str);
}

pub fn console_print(msg: &str) {
    #[cfg(not(target_arch = "wasm32"))]
    println!("{}", msg);
    #[cfg(target_arch = "wasm32")]
    unsafe {
        sys_print(&format!("{}", msg))
    };
}
