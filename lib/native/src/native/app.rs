#[cfg(not(target_os = "android"))]
pub type NativeApp = ();

#[cfg(target_os = "android")]
pub use winit::platform::android::activity::AndroidApp as NativeApp;

pub const MIN_WINDOW_WIDTH: u32 = 50;
pub const MIN_WINDOW_HEIGHT: u32 = 50;
