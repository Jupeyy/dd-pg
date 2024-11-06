use crossbeam::channel::Receiver;

use crate::{stream_sample::StreamSample, traits::MicrophoneStreamRaii};

pub struct MicrophoneStream {
    pub opus_receiver: Receiver<StreamSample>,
    pub(crate) inner: Box<dyn MicrophoneStreamRaii>,
}

impl MicrophoneStream {
    pub fn split(self) -> (Box<dyn MicrophoneStreamRaii>, Receiver<StreamSample>) {
        (self.inner, self.opus_receiver)
    }
}
