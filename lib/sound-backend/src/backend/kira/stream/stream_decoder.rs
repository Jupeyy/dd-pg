use std::sync::Arc;

use kira::sound::streaming::Decoder;
use sound::stream::{self, DecodeError, StreamFrame};

pub struct StreamDecoder {
    pub(super) decoder: Arc<dyn stream::StreamDecoder>,
}

impl Decoder for StreamDecoder {
    type Error = anyhow::Error;

    fn sample_rate(&self) -> u32 {
        self.decoder.sample_rate()
    }

    fn num_frames(&self) -> usize {
        self.decoder.num_frames()
    }

    fn decode(&mut self) -> Result<Vec<kira::Frame>, Self::Error> {
        let samples = match self.decoder.decode() {
            Ok(samples) => samples,
            Err(DecodeError::MustGenerateEmpty(count)) => vec![StreamFrame::default(); count],
            Err(DecodeError::Err(err)) => Err(err)?,
        };
        Ok(samples
            .into_iter()
            .map(|frame| kira::Frame {
                left: frame.left,
                right: frame.right,
            })
            .collect())
    }

    fn seek(&mut self, index: usize) -> Result<usize, Self::Error> {
        self.decoder.seek(index)
    }
}
