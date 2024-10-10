/// Settings that are given to the encoder
#[derive(Debug, Clone)]
pub struct EncoderSettings {
    /// Frames per second
    pub fps: u32,
    /// "Constant Rate Factor" for x264.
    /// Where 0 is lossless and 51 is the worst.
    /// 18 is default.
    pub crf: u8,
    /// Width of the video
    pub width: u32,
    /// Height of the video
    pub height: u32,
    /// The hardware acceleration to use during video encoding.
    /// This setting is highly OS dependent.
    pub hw_accel: String,
    /// Max number of CPU threads the encoders should use.
    pub max_threads: u64,
    /// Sample rate for audio.
    /// It's __strongly__ recommended that this is a multiple of
    /// [`Self::fps`].
    pub sample_rate: u32,
}
