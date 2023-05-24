use graphics::graphics::Graphics;

pub mod graphics;

extern "C" {
    fn host_raw_bytes_add_u64(byte_stream: u64, byte_count: u8);
    fn host_raw_bytes_add_u64_2(byte_stream: u64, byte_count: u8);
    fn host_raw_bytes_add_u64_3(byte_stream: u64, byte_count: u8);
    fn host_raw_bytes_add_u64_4(byte_stream: u64, byte_count: u8);
    fn host_println();
}

extern "Rust" {
    fn mod_main(graphics: &mut Graphics);
}

pub fn push_raw_bytes_array(index: usize, stream_el: u64, byte_count: u8) {
    match index {
        0 => unsafe { host_raw_bytes_add_u64(stream_el, byte_count) },
        1 => unsafe { host_raw_bytes_add_u64_2(stream_el, byte_count) },
        2 => unsafe { host_raw_bytes_add_u64_3(stream_el, byte_count) },
        3 => unsafe { host_raw_bytes_add_u64_4(stream_el, byte_count) },
        _ => panic!("not implemented yet."),
    }
}

pub fn upload_bytes(index: usize, bytes: &[u8]) {
    bytes.chunks(std::mem::size_of::<u64>()).for_each(|chunk| {
        let mut chunk_full: [u8; std::mem::size_of::<u64>()] = [0; std::mem::size_of::<u64>()];
        chunk_full[0..chunk.len()].copy_from_slice(chunk);
        let stream_el = u64::from_le_bytes(chunk_full);
        push_raw_bytes_array(index, stream_el, chunk.len() as u8);
    });
}

pub fn println(text: &str) {
    upload_bytes(0, text.as_bytes());
    unsafe { host_println() };
}

static mut GRAPHICS: once_cell::unsync::Lazy<Graphics> =
    once_cell::unsync::Lazy::new(|| Graphics::new());

#[no_mangle]
pub fn api_run() {
    unsafe { mod_main(&mut GRAPHICS) };
}
