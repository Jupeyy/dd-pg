use std::sync::Mutex;

use async_trait::async_trait;

use super::{connection::NetworkConnectionId, plugins::NetworkPluginPacket};

/// A network plugin, that can collects packets
/// to generate a dictionary from network traffic.
/// Note: when the network is dropped this will write the dict to disk,
/// this generally is an expensive operation, you should only
/// activate this plugin when you seriously plan to train a dictionary.
/// Also this uses lot of RAM.
/// You should put this plugin **BEFORE** packet compression plugins.
#[derive(Debug)]
pub struct ZstdNetworkDictTrainer {
    sent_packets: Mutex<Vec<Vec<u8>>>,
    recv_packets: Mutex<Vec<Vec<u8>>>,

    output_size: usize,
}

impl Default for ZstdNetworkDictTrainer {
    fn default() -> Self {
        Self::new(1024 * 64)
    }
}

impl ZstdNetworkDictTrainer {
    pub fn new(output_size: usize) -> Self {
        Self {
            sent_packets: Mutex::new(Vec::with_capacity(1024 * 1024)),
            recv_packets: Mutex::new(Vec::with_capacity(1024 * 1024)),
            output_size,
        }
    }
}

#[async_trait]
impl NetworkPluginPacket for ZstdNetworkDictTrainer {
    async fn prepare_write(
        &self,
        _id: &NetworkConnectionId,
        buffer: &mut Vec<u8>,
    ) -> anyhow::Result<()> {
        self.sent_packets.lock().unwrap().push(buffer.clone());
        Ok(())
    }
    async fn prepare_read(
        &self,
        _id: &NetworkConnectionId,
        buffer: &mut Vec<u8>,
    ) -> anyhow::Result<()> {
        self.recv_packets.lock().unwrap().push(buffer.clone());

        Ok(())
    }
}

impl Drop for ZstdNetworkDictTrainer {
    fn drop(&mut self) {
        let sent_packets = std::mem::take(&mut *self.sent_packets.lock().unwrap());
        let recv_packets = std::mem::take(&mut *self.recv_packets.lock().unwrap());

        if let Err(err) = zstd::dict::from_samples(&sent_packets, self.output_size)
            .and_then(|dict| std::fs::write("sent_packets", dict))
        {
            log::error!("failed to train dictionary for sent packets: {err}");
        }
        if let Err(err) = zstd::dict::from_samples(&recv_packets, self.output_size)
            .and_then(|dict| std::fs::write("recv_packets", dict))
        {
            log::error!("failed to train dictionary for recv packets: {err}");
        }
    }
}
