#[cfg(feature = "ffmpeg")]
pub mod encoder;
#[cfg(feature = "ffmpeg")]
mod ffmpeg;

pub mod stub;
pub mod traits;
pub mod types;

#[cfg(feature = "ffmpeg")]
pub type AvEncoder = encoder::FfmpegEncoder;
#[cfg(not(feature = "ffmpeg"))]
pub type AvEncoder = stub::StubEncoder;
