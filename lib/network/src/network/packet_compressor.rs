use std::io::{Read, Write};

use anyhow::anyhow;
use async_trait::async_trait;
use pool::mt_pool::Pool;
use serde::{Deserialize, Serialize};

use super::{connection::NetworkConnectionID, plugins::NetworkPluginPacket};

#[derive(Debug, Serialize, Deserialize)]
struct CompressHeader {
    size: usize,
    is_compressed: bool,
}

/// A network plugin, that can compress packets using brotli.
/// Good in size, bad in speed
#[derive(Debug)]
pub struct BrotliNetworkPacketCompressor {
    helper_pool: Pool<Vec<u8>>,
}

impl BrotliNetworkPacketCompressor {
    pub fn new() -> Self {
        Self {
            helper_pool: Pool::with_capacity(64),
        }
    }
}

#[async_trait]
impl NetworkPluginPacket for BrotliNetworkPacketCompressor {
    async fn prepare_write(
        &self,
        _id: &NetworkConnectionID,
        buffer: &mut Vec<u8>,
    ) -> anyhow::Result<()> {
        let mut helper = self.helper_pool.new();
        let helper: &mut Vec<_> = helper.as_mut();

        brotli::CompressorWriter::new(&mut *helper, 4096, 8, 22).write_all(buffer)?;

        let header = CompressHeader {
            size: helper.len().min(buffer.len()),
            is_compressed: helper.len() < buffer.len(),
        };

        let mut size_helper = self.helper_pool.new();
        let size_helper: &mut Vec<_> = size_helper.as_mut();
        bincode::serde::encode_into_std_write(&header, size_helper, bincode::config::standard())?;
        if header.is_compressed {
            size_helper.append(helper);
        } else {
            size_helper.append(buffer);
        }

        std::mem::swap(buffer, size_helper);
        Ok(())
    }
    async fn prepare_read(
        &self,
        _id: &NetworkConnectionID,
        buffer: &mut Vec<u8>,
    ) -> anyhow::Result<()> {
        let (header, read_size) = bincode::serde::decode_from_slice::<CompressHeader, _>(
            buffer,
            bincode::config::standard(),
        )?;

        if header.is_compressed {
            let mut helper = self.helper_pool.new();
            let helper: &mut Vec<_> = helper.as_mut();

            brotli::Decompressor::new(
                buffer
                    .get(read_size..read_size + header.size)
                    .ok_or_else(|| anyhow!("header slice out of bounds"))?,
                4096,
            )
            .read_to_end(helper)?;

            std::mem::swap(buffer, helper);
        } else {
            buffer.splice(0..read_size, []);
        }

        Ok(())
    }
}

/// A network plugin, that can compress packets using zstd.
/// Good in speed, size depends on dictionary.
#[derive(Debug)]
pub struct ZstdNetworkPacketCompressor {
    helper_pool: Pool<Vec<u8>>,
}

impl ZstdNetworkPacketCompressor {
    pub fn new() -> Self {
        Self {
            helper_pool: Pool::with_capacity(64),
        }
    }
}

#[async_trait]
impl NetworkPluginPacket for ZstdNetworkPacketCompressor {
    async fn prepare_write(
        &self,
        _id: &NetworkConnectionID,
        buffer: &mut Vec<u8>,
    ) -> anyhow::Result<()> {
        let mut helper = self.helper_pool.new();
        let helper: &mut Vec<_> = helper.as_mut();

        let mut encoder = zstd::Encoder::new(&mut *helper, 0)?;
        encoder.write_all(buffer)?;
        encoder.finish()?;

        let header = CompressHeader {
            size: helper.len().min(buffer.len()),
            is_compressed: helper.len() < buffer.len(),
        };

        let mut size_helper = self.helper_pool.new();
        let size_helper: &mut Vec<_> = size_helper.as_mut();
        bincode::serde::encode_into_std_write(&header, size_helper, bincode::config::standard())?;
        if header.is_compressed {
            size_helper.append(helper);
        } else {
            size_helper.append(buffer);
        }

        std::mem::swap(buffer, size_helper);
        Ok(())
    }
    async fn prepare_read(
        &self,
        _id: &NetworkConnectionID,
        buffer: &mut Vec<u8>,
    ) -> anyhow::Result<()> {
        let (header, read_size) = bincode::serde::decode_from_slice::<CompressHeader, _>(
            buffer,
            bincode::config::standard(),
        )?;

        if header.is_compressed {
            let mut helper = self.helper_pool.new();
            let helper: &mut Vec<_> = helper.as_mut();

            let mut decoder = zstd::Decoder::new(
                buffer
                    .get(read_size..read_size + header.size)
                    .ok_or_else(|| anyhow!("header slice out of bounds"))?,
            )?;
            decoder.read_to_end(helper)?;
            decoder.finish();

            std::mem::swap(buffer, helper);
        } else {
            buffer.splice(0..read_size, []);
        }

        Ok(())
    }
}

/// A network plugin, that can compress packets.
pub type DefaultNetworkPacketCompressor = ZstdNetworkPacketCompressor;
