use config::config::{AtomicGfxDebugModes, GfxDebugModes};

/************************
 * LOGGING
 ************************/

#[must_use]
pub fn is_verbose_mode(dbg_gfx: GfxDebugModes) -> bool {
    let val = dbg_gfx;
    val == GfxDebugModes::Verbose || val == GfxDebugModes::All
}

#[must_use]
pub fn is_verbose(dbg_gfx: &AtomicGfxDebugModes) -> bool {
    let val = dbg_gfx.load(std::sync::atomic::Ordering::Relaxed);
    val == GfxDebugModes::Verbose || val == GfxDebugModes::All
}
