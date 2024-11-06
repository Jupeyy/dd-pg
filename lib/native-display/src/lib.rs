/// The backend for the native display handle
#[derive(Debug, Clone, Copy)]
pub enum NativeDisplayBackend {
    Windows,
    Wayland,
    Xlib,
    Android,
    Apple,

    Unknown(raw_window_handle::RawDisplayHandle),
}

pub fn get_native_display_backend() -> anyhow::Result<NativeDisplayBackend> {
    #[cfg(target_os = "android")]
    {
        Ok(NativeDisplayBackend::Android)
    }
    #[cfg(target_os = "linux")]
    {
        Ok(
            if std::env::var("WAYLAND_DISPLAY")
                .ok()
                .filter(|var| !var.is_empty())
                .or_else(|| std::env::var("WAYLAND_SOCKET").ok())
                .filter(|var| !var.is_empty())
                .is_some()
            {
                NativeDisplayBackend::Wayland
            } else if std::env::var("DISPLAY")
                .map(|var| !var.is_empty())
                .unwrap_or(false)
            {
                NativeDisplayBackend::Xlib
            } else {
                return Err(anyhow::anyhow!("Unknown linux display backend"));
            },
        )
    }
    #[cfg(target_os = "windows")]
    {
        Ok(NativeDisplayBackend::Windows)
    }
    #[cfg(target_os = "macos")]
    {
        Ok(NativeDisplayBackend::Apple)
    }
}
