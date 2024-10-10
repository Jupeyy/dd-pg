pub(crate) fn as_res(code: i32) -> Result<(), ffmpeg_next::Error> {
    match code {
        0 => Ok(()),
        _ => Err(ffmpeg_next::Error::from(code)),
    }
}
pub(crate) fn non_null<T>(ptr: *mut T) -> Result<*mut T, ffmpeg_next::Error> {
    if ptr.is_null() {
        Err(ffmpeg_next::Error::Unknown)
    } else {
        Ok(ptr)
    }
}
