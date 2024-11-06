#[cfg(feature = "cpal_opus")]
mod cpal_opus;
#[cfg(not(feature = "cpal_opus"))]
mod null;

pub mod stream;
pub mod stream_sample;
pub mod traits;
pub mod types;

#[cfg(feature = "cpal_opus")]
pub type AnalyzeStream = cpal_opus::analyze_stream::AnalyzeStream;
#[cfg(feature = "cpal_opus")]
pub type SoundStream = cpal_opus::sound_stream::SoundStream;
#[cfg(feature = "cpal_opus")]
pub type MicrophoneManager = cpal_opus::manager::MicrophoneManager;

#[cfg(not(feature = "cpal_opus"))]
pub type AnalyzeStream = null::analyze_stream::AnalyzeStream;
#[cfg(not(feature = "cpal_opus"))]
pub type SoundStream = null::sound_stream::SoundStream;
#[cfg(not(feature = "cpal_opus"))]
pub type MicrophoneManager = null::manager::Manager;
