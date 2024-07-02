use std::sync::Arc;

use base::system::SystemTime;
use raw_window_handle::RawDisplayHandle;
use winit::monitor::MonitorHandle;

use crate::input::InputEventHandler;

use self::winit_wrapper::WinitWrapper;

mod winit_wrapper;

pub use winit::dpi::PhysicalSize;
pub use winit::event::DeviceId;
pub use winit::event::MouseButton;
pub use winit::event::MouseScrollDelta;
pub use winit::window::Window;
pub use winit::{
    event::WindowEvent,
    keyboard::{KeyCode, PhysicalKey},
};

pub trait NativeImpl {
    /// Grabs the mouse.
    /// If a direct grab fails, it queues the grab to a later cycle.
    /// This function chaches the grab mode and can safely be called
    /// every frame.
    fn mouse_grab(&mut self);
    /// Show or hide the cursor.
    /// This function chaches the cursor state and can safely be called
    /// every frame.
    fn toggle_cursor(&mut self, show: bool);
    /// Change the window config.
    /// Automatically only applies _actual_ changes.
    fn set_window_config(&mut self, wnd: NativeWindowOptions) -> anyhow::Result<()>;
    fn borrow_window(&self) -> &Window;
    fn monitors(&self) -> Vec<MonitorHandle>;
    fn window_options(&self) -> NativeWindowOptions;
    fn quit(&self);
    fn start_arguments(&self) -> &Vec<String>;
}

pub trait FromNativeImpl: InputEventHandler {
    fn run(&mut self, native: &mut dyn NativeImpl);
    fn resized(&mut self, native: &mut dyn NativeImpl, new_width: u32, new_height: u32);
    /// The window options changed, usually the implementor does not need to do anything.
    /// But if it wants to serialize the current options it can do so.
    fn window_options_changed(&mut self, wnd: NativeWindowOptions);
    fn destroy(self);
}

pub trait FromNativeLoadingImpl<L>
where
    Self: Sized,
{
    fn load_with_display_handle(
        loading: &mut L,
        display_handle: RawDisplayHandle,
    ) -> anyhow::Result<()>;
    fn new(loading: L, native: &mut dyn NativeImpl) -> anyhow::Result<Self>;
}

#[derive(Debug)]
pub struct NativeWindowMonitorDetails {
    pub name: String,
    pub size: PhysicalSize<u32>,
}

#[derive(Debug)]
pub struct NativeWindowOptions {
    pub fullscreen: bool,
    /// if fullscreen is `false` & maximized is `true` & decorated is `false`
    /// => borderless fullscreen
    pub decorated: bool,
    pub maximized: bool,
    pub width: u32,
    pub height: u32,
    pub refresh_rate_milli_hertz: u32,
    pub monitor: Option<NativeWindowMonitorDetails>,
}

#[derive(Debug)]
pub struct NativeCreateOptions<'a> {
    pub do_bench: bool,
    pub dbg_input: bool,
    pub title: String,
    pub sys: &'a Arc<SystemTime>,
    pub start_arguments: Vec<String>,
    pub window: NativeWindowOptions,
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
