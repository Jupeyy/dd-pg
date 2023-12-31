use std::sync::Arc;

use base::system::SystemTime;
use winit::window::{raw_window_handle::RawDisplayHandle, Window};

use crate::input::InputEventHandler;

use self::winit_wrapper::WinitWrapper;

mod winit_wrapper;

pub trait NativeImpl {
    /// grabs the mouse
    /// if a direct grab fails, it queues the grab to a later cycle
    fn mouse_grab(&mut self);
    fn borrow_window(&self) -> &Window;
    fn quit(&self);
    fn start_arguments(&self) -> &Vec<String>;
}

pub trait FromNativeImpl: InputEventHandler {
    fn run(&mut self, native: &mut dyn NativeImpl);
    fn resized(&mut self, native: &mut dyn NativeImpl, new_width: u32, new_height: u32);
    fn destroy(self);
}

pub trait FromNativeLoadingImpl<L>
where
    Self: Sized,
{
    fn load_with_display_handle(
        loading: &mut L,
        raw_display_handle: RawDisplayHandle,
    ) -> anyhow::Result<()>;
    fn new(loading: L, native: &mut dyn NativeImpl) -> anyhow::Result<Self>;
}

#[derive(Debug)]
pub struct NativeCreateOptions<'a> {
    pub do_bench: bool,
    pub dbg_input: bool,
    pub title: String,
    pub sys: &'a Arc<SystemTime>,
    pub start_arguments: Vec<String>,
}

pub struct Native {}

impl Native {
    pub fn run_loop<F, L>(
        mut native_user_loading: L,
        native_options: NativeCreateOptions,
    ) -> anyhow::Result<()>
    where
        F: FromNativeImpl + FromNativeLoadingImpl<L> + 'static,
    {
        let mut winit_wrapper =
            WinitWrapper::new::<F, L>(native_options, &mut native_user_loading)?;
        let native_user = F::new(native_user_loading, &mut winit_wrapper.window)?;
        winit_wrapper.run(native_user)
    }
}
