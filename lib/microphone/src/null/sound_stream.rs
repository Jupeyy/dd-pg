use std::sync::Arc;

use anyhow::anyhow;
use crossbeam::channel::Receiver;
use hiarc::Hiarc;
use sound::stream::StreamDecoder;

use crate::{
    stream::MicrophoneStream, stream_sample::StreamSample, traits::MicrophoneStreamRaii,
    types::SoundStreamsettings,
};

#[derive(Debug, Hiarc, Default, Clone, Copy)]
pub struct FakeStream;

impl StreamDecoder for FakeStream {
    fn sample_rate(&self) -> u32 {
        0
    }

    fn num_frames(&self) -> usize {
        0
    }

    fn decode(&self) -> Result<Vec<sound::stream::StreamFrame>, sound::stream::DecodeError> {
        Err(sound::stream::DecodeError::Err(anyhow!(
            "null stream cannot decode."
        )))
    }

    fn seek(&self, _index: usize) -> Result<usize, anyhow::Error> {
        Err(anyhow!("null stream cannot seek."))
    }
}

#[derive(Debug, Hiarc, Default)]
pub struct SoundStream;

impl SoundStream {
    pub fn from_receiver(
        _opus_receiver: Receiver<StreamSample>,
        _inner: Option<Box<dyn MicrophoneStreamRaii>>,
        _settings: SoundStreamsettings,
    ) -> Self {
        Self
    }

    pub fn new(_microphone: MicrophoneStream, _settings: SoundStreamsettings) -> Self {
        Self
    }

    pub fn stream(&self) -> Arc<FakeStream> {
        Arc::new(FakeStream)
    }
}
