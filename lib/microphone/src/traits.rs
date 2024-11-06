use crate::{
    stream::MicrophoneStream,
    types::{MicrophoneDevices, MicrophoneHosts, MicrophoneNoiseFilterSettings},
};

/// Just a RAII object that keeps the stream running
pub trait MicrophoneStreamRaii {}

pub trait Microphone {
    fn hosts(&self) -> MicrophoneHosts;

    fn devices(&self, host: &str) -> anyhow::Result<MicrophoneDevices>;

    /// When the stream handle is dropped, the recording ends.
    fn stream_opus(
        &self,
        host: &str,
        device: &str,
        settings: MicrophoneNoiseFilterSettings,
    ) -> anyhow::Result<MicrophoneStream>;
}
