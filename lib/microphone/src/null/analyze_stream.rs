use std::sync::{Arc, RwLock};

use hiarc::Hiarc;

use crate::stream::MicrophoneStream;

#[derive(Debug, Hiarc, Default)]
pub struct AnalyzeStream {
    pub cur_loudest: Arc<RwLock<f32>>,
}

impl AnalyzeStream {
    pub fn new(_microphone: MicrophoneStream) -> Self {
        Self::default()
    }
}
