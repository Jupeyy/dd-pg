use anyhow::anyhow;
use hiarc::Hiarc;

use crate::{
    traits::Microphone,
    types::{MicrophoneDevices, MicrophoneHosts},
};

#[derive(Debug, Hiarc, Default)]
pub struct Manager;

impl Microphone for Manager {
    fn hosts(&self) -> MicrophoneHosts {
        MicrophoneHosts {
            default: "null".to_string(),
            hosts: Default::default(),
        }
    }

    fn devices(&self, _host: &str) -> anyhow::Result<MicrophoneDevices> {
        Err(anyhow!("null host has no devices."))
    }

    fn stream_opus(
        &self,
        _host: &str,
        _device: &str,
        _settings: crate::types::MicrophoneNoiseFilterSettings,
    ) -> anyhow::Result<crate::stream::MicrophoneStream> {
        Err(anyhow!("null can no start streams."))
    }
}
