use config::config::{AtomicEDebugGFXModes, EDebugGFXModes};

/************************
 * LOGGING
 ************************/

#[must_use]
pub fn is_verbose_mode(dbg_gfx: EDebugGFXModes) -> bool {
    let val = dbg_gfx;
    val == EDebugGFXModes::Verbose || val == EDebugGFXModes::All
}

#[must_use]
pub fn is_verbose(dbg_gfx: &AtomicEDebugGFXModes) -> bool {
    let val = dbg_gfx.load(std::sync::atomic::Ordering::Relaxed);
    val == EDebugGFXModes::Verbose || val == EDebugGFXModes::All
}
