use std::sync::Arc;

use base::system::SystemTime;
use winit::window::Window;

use crate::input::InputEventHandler;

use self::winit_wrapper::WinitWrapper;

mod winit_wrapper;

pub trait NativeImpl {
    /// grabs the mouse
    /// if a direct grab fails, it queues the grab to a later cycle
    fn mouse_grab(&mut self);
    fn borrow_window(&self) -> &Window;
}

pub trait FromNativeImpl: InputEventHandler {
    fn run(&mut self, native: &mut dyn NativeImpl);
    fn resized(&mut self, native: &mut dyn NativeImpl, new_width: u32, new_height: u32);
    fn destroy(self);
}

pub trait FromNativeLoadingImpl<L> {
    fn new(loading: L, native: &mut dyn NativeImpl) -> Self;
}

#[derive(Debug)]
pub struct NativeCreateOptions<'a> {
    pub do_bench: bool,
    pub title: String,
    pub sys: &'a Arc<SystemTime>,
}

pub struct Native {}

impl Native {
    pub fn run_loop<F, L>(native_user_loading: L, native_options: NativeCreateOptions) -> !
    where
        F: FromNativeImpl + FromNativeLoadingImpl<L> + 'static,
    {
        let mut winit_wrapper = WinitWrapper::new(native_options);
        let native_user = F::new(native_user_loading, &mut winit_wrapper.window);
        winit_wrapper.run(native_user)
    }
}
