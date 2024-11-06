use std::fmt::Debug;

use hiarc::Hiarc;

#[derive(Debug, Hiarc, Default, Clone, Copy)]
pub struct StreamFrame {
    pub left: f32,
    pub right: f32,
}

pub enum DecodeError {
    /// Generate `usize` empty samples
    MustGenerateEmpty(usize),
    Err(anyhow::Error),
}

pub trait StreamDecoder: 'static + Debug + Send + Sync {
    // Required methods
    fn sample_rate(&self) -> u32;
    fn num_frames(&self) -> usize;
    fn decode(&self) -> Result<Vec<StreamFrame>, DecodeError>;
    fn seek(&self, index: usize) -> Result<usize, anyhow::Error>;
}
