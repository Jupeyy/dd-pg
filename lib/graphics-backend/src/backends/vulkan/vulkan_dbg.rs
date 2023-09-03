use std::sync::atomic::AtomicU8;

use config::config::EDebugGFXModes;

/************************
 * LOGGING
 ************************/

#[must_use]
pub fn is_verbose_mode(dbg_gfx: EDebugGFXModes) -> bool {
    let val = dbg_gfx;
    return val == EDebugGFXModes::Verbose || val == EDebugGFXModes::All;
}

#[must_use]
pub fn is_verbose(dbg_gfx: &AtomicU8) -> bool {
    let val = dbg_gfx.load(std::sync::atomic::Ordering::Relaxed);
    return val == EDebugGFXModes::Verbose as u8 || val == EDebugGFXModes::All as u8;
}
