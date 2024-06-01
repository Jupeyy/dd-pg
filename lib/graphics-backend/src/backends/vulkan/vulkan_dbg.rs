use config::config::{AtomicGFXDebugModes, GFXDebugModes};

/************************
 * LOGGING
 ************************/

#[must_use]
pub fn is_verbose_mode(dbg_gfx: GFXDebugModes) -> bool {
    let val = dbg_gfx;
    val == GFXDebugModes::Verbose || val == GFXDebugModes::All
}

#[must_use]
pub fn is_verbose(dbg_gfx: &AtomicGFXDebugModes) -> bool {
    let val = dbg_gfx.load(std::sync::atomic::Ordering::Relaxed);
    val == GFXDebugModes::Verbose || val == GFXDebugModes::All
}
