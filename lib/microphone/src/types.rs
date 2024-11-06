use hiarc::Hiarc;
use serde::{Deserialize, Serialize};

#[derive(Debug, Hiarc, Clone, Copy, Serialize, Deserialize)]
pub struct NoiseGateSettings {
    pub open_threshold: f64,
    pub close_threshold: f64,
}

impl Default for NoiseGateSettings {
    fn default() -> Self {
        Self {
            open_threshold: -36.0,
            close_threshold: -54.0,
        }
    }
}

#[derive(Debug, Hiarc, Clone, Serialize, Deserialize)]
pub struct MicrophoneHosts {
    pub hosts: Vec<String>,
    pub default: String,
}

#[derive(Debug, Hiarc, Default, Clone, Serialize, Deserialize)]
pub struct MicrophoneDevices {
    pub devices: Vec<String>,
    pub default: Option<String>,
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub struct NoiseFilterSettings {
    /// in db
    pub attenuation: f64,
    /// in db
    pub processing_threshold: f64,
}

impl Default for NoiseFilterSettings {
    fn default() -> Self {
        Self {
            attenuation: 100.0,
            processing_threshold: -10.0,
        }
    }
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub struct MicrophoneNoiseFilterSettings {
    /// Has to be `Some` to use a noise filter at all
    pub nf: Option<NoiseFilterSettings>,
    pub noise_gate: NoiseGateSettings,
    /// Microphone boost in db
    pub boost: f64,
}

impl Default for MicrophoneNoiseFilterSettings {
    fn default() -> Self {
        Self {
            nf: Some(Default::default()),
            noise_gate: Default::default(),
            boost: 0.0,
        }
    }
}

#[derive(Debug, Default, Copy, Clone)]
pub struct SoundStreamsettings {
    /// Has to be `Some` to use a noise filter at all
    pub nf: Option<NoiseFilterSettings>,
    pub noise_gate: Option<NoiseGateSettings>,
    /// Boost in db
    pub boost: f64,
}
