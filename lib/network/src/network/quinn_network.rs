use std::{
    collections::{HashMap, VecDeque},
    future::Future,
    net::SocketAddr,
    pin::Pin,
    sync::Arc,
};

use anyhow::anyhow;
use pool::mt_datatypes::PoolVec;
use tokio::io::AsyncWriteExt;

use super::{
    network::{
        Network, NetworkClientInitOptions, NetworkConnectingInterface, NetworkConnectionInterface,
        NetworkEndpointInterface, NetworkInOrderChannel, NetworkServerInitOptions,
    },
    quinnminimal::{make_client_endpoint, make_server_endpoint},
};

#[derive(Default)]
pub struct QuinnNetworkConnectingWrapperChannel {
    in_order_packets: VecDeque<PoolVec<u8>>,
    open_bi: Option<(quinn::SendStream, quinn::RecvStream)>,
}

#[derive(Clone)]
pub struct QuinnNetworkConnectionWrapper {
    con: quinn::Connection,
    channels: Arc<
        tokio::sync::Mutex<
            HashMap<
                NetworkInOrderChannel,
                Arc<tokio::sync::Mutex<QuinnNetworkConnectingWrapperChannel>>,
            >,
        >,
    >,
}

impl QuinnNetworkConnectionWrapper {
    async fn write_bytes_chunked(
        send_stream: &mut quinn::SendStream,
        packet: PoolVec<u8>,
    ) -> anyhow::Result<()> {
        let packet_len = packet.len() as u64;
        let send_buffer = [packet_len.to_le_bytes().to_vec(), packet.take()].concat();
        let written_bytes = send_stream.write_all(send_buffer.as_slice()).await;
        if let Err(err) = written_bytes {
            Err(anyhow!(format!("packet write failed: {}", err.to_string())))
        } else {
            match send_stream.flush().await {
                Ok(_) => Ok(()),
                Err(err) => Err(anyhow!(format!("packet flush failed: {}", err.to_string()))),
            }
        }
    }
}

const READ_LIMIT_SIZE: usize = 1024 as usize * 1024 * 1024;

#[async_trait::async_trait]
impl NetworkConnectionInterface for QuinnNetworkConnectionWrapper {
    async fn send_unreliable_unordered(&self, data: PoolVec<u8>) -> anyhow::Result<()> {
        let pack_bytes = bytes::Bytes::copy_from_slice(&data[..]);
        let res = self.con.send_datagram(pack_bytes)?;
        Ok(res)
    }

    async fn read_unreliable_unordered(&self) -> anyhow::Result<Vec<u8>> {
        let res = self.con.read_datagram().await;
        match res {
            Ok(res) => Ok(res.to_vec()),
            Err(err) => Err(anyhow!(err.to_string())),
        }
    }

    async fn send_unordered_reliable(&self, data: PoolVec<u8>) -> anyhow::Result<()> {
        let uni = self.con.open_uni().await;
        if let Ok(mut stream) = uni {
            let written_bytes = stream.write_all(&data.as_slice()).await;
            if let Err(_written_bytes) = written_bytes {
                Err(anyhow!("packet write failed."))
            } else {
                let finish_res = stream.finish().await;
                if let Err(err) = finish_res {
                    Err(anyhow!(format!(
                        "packet finish failed: {}",
                        err.to_string()
                    )))
                } else {
                    Ok(())
                }
            }
        } else {
            Err(anyhow!(format!(
                "sent stream err: {}",
                uni.unwrap_err().to_string()
            )))
        }
    }

    async fn read_unordered_reliable<
        F: FnOnce(anyhow::Result<Vec<u8>>) -> tokio::task::JoinHandle<()> + Send + 'static,
    >(
        &self,
        on_data: F,
    ) -> anyhow::Result<()> {
        let uni = self.con.accept_uni().await;
        match uni {
            Ok(mut recv_stream) => {
                tokio::spawn(async move {
                    match recv_stream.read_to_end(READ_LIMIT_SIZE).await {
                        Ok(read_res) => {
                            on_data(Ok(read_res)).await.unwrap_or_else(|_| {
                                // ignore
                            });
                        }
                        Err(read_err) => {
                            on_data(Err(anyhow!(format!(
                                "connection stream acception failed {}",
                                read_err
                            ))))
                            .await
                            .unwrap();
                        }
                    }
                });
                Ok(())
            }
            Err(recv_err) => Err(anyhow!(format!(
                "connection stream acception failed {}",
                recv_err
            ))),
        }
    }

    async fn push_ordered_reliable_packet_in_order(
        &self,
        data: PoolVec<u8>,
        channel: NetworkInOrderChannel,
    ) {
        let mut channels = self.channels.lock().await;
        if !channels.contains_key(&channel) {
            channels.insert(channel, Default::default());
        }
        let cur_channel = channels.get_mut(&channel).unwrap().clone();
        drop(channels);
        cur_channel.lock().await.in_order_packets.push_back(data);
    }

    async fn send_one_ordered_reliable(
        &self,
        channel: NetworkInOrderChannel,
    ) -> anyhow::Result<()> {
        let cur_channel = self.channels.lock().await.get_mut(&channel).cloned();
        if let Some(cur_channel) = cur_channel {
            let mut cur_channel = cur_channel.lock().await;
            let packet_res = cur_channel.in_order_packets.pop_front();
            if let Some(packet) = packet_res {
                if let Some((send_stream, _)) = cur_channel.open_bi.as_mut() {
                    Self::write_bytes_chunked(send_stream, packet).await
                } else {
                    match self.con.open_bi().await {
                        Ok((send, recv)) => {
                            cur_channel.open_bi = Some((send, recv));
                            Self::write_bytes_chunked(
                                &mut cur_channel.open_bi.as_mut().unwrap().0,
                                packet,
                            )
                            .await
                        }
                        Err(err) => Err(anyhow!(err.to_string())),
                    }
                }
            } else {
                Err(anyhow!("No packet was queued."))
            }
        } else {
            Err(anyhow!("Channel did not exist."))
        }
    }

    async fn read_ordered_reliable<
        F: Fn(anyhow::Result<Vec<u8>>) -> tokio::task::JoinHandle<()> + Send + Sync + 'static,
    >(
        &self,
        on_data: F,
    ) -> anyhow::Result<()> {
        match self.con.accept_bi().await {
            Ok((_, mut recv_stream)) => {
                tokio::spawn(async move {
                    let mut len_buff: [u8; std::mem::size_of::<u64>()] = Default::default();
                    'read_loop: loop {
                        match recv_stream.read_exact(&mut len_buff).await {
                            Ok(_) => {
                                let read_buff_len = u64::from_le_bytes(len_buff);
                                if read_buff_len > READ_LIMIT_SIZE as u64 {
                                    on_data(Err(anyhow!("read size exceeded max length.",)))
                                        .await
                                        .unwrap();
                                    break 'read_loop;
                                } else {
                                    let mut read_buff: Vec<u8> = Vec::new();
                                    read_buff.resize(read_buff_len as usize, Default::default());

                                    match recv_stream.read_exact(read_buff.as_mut()).await {
                                        Ok(_) => {
                                            on_data(Ok(read_buff)).await.unwrap();
                                        }
                                        Err(err) => {
                                            on_data(Err(anyhow!(err.to_string()))).await.unwrap();
                                            break 'read_loop;
                                        }
                                    }
                                }
                            }
                            Err(err) => {
                                on_data(Err(anyhow!(err.to_string()))).await.unwrap();
                                break 'read_loop;
                            }
                        }
                    }
                });
                Ok(())
            }
            Err(err) => {
                return Err(anyhow!(err.to_string()));
            }
        }
    }

    fn close(&self, error_code: quinn::VarInt, reason: &[u8]) {
        self.con.close(error_code, reason)
    }
}

pub struct QuinnNetworkConnectingWrapper {
    connecting: quinn::Connecting,
    allow_05_rrt: bool,
}

impl Future for QuinnNetworkConnectingWrapper {
    type Output = Result<QuinnNetworkConnectionWrapper, String>;

    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let con = Pin::new(&mut self.connecting).poll(cx);
        con.map(|f| match f {
            Ok(connection) => Ok(QuinnNetworkConnectionWrapper {
                con: connection,
                channels: Default::default(),
            }),
            Err(err) => Err(err.to_string()),
        })
    }
}

impl NetworkConnectingInterface<QuinnNetworkConnectionWrapper> for QuinnNetworkConnectingWrapper {
    fn try_fast_unwrap(self) -> Result<QuinnNetworkConnectionWrapper, Self> {
        if self.allow_05_rrt {
            match self.connecting.into_0rtt() {
                Ok((con, _)) => Ok(QuinnNetworkConnectionWrapper {
                    con: con,
                    channels: Default::default(),
                }),
                Err(err) => Err(QuinnNetworkConnectingWrapper {
                    connecting: err,
                    allow_05_rrt: true,
                }),
            }
        } else {
            Err(self)
        }
    }
}

#[derive(Clone)]
pub struct QuinnEndpointWrapper {
    endpoint: quinn::Endpoint,
    is_server: bool,
    allows_05_rtt: bool,
}

#[async_trait::async_trait]
impl NetworkEndpointInterface<QuinnNetworkConnectingWrapper> for QuinnEndpointWrapper {
    fn connect(
        &self,
        addr: std::net::SocketAddr,
        server_name: &str,
    ) -> anyhow::Result<QuinnNetworkConnectingWrapper> {
        let res = self.endpoint.connect(addr, server_name)?;
        Ok(QuinnNetworkConnectingWrapper {
            connecting: res,
            allow_05_rrt: false,
        })
    }

    fn close(&self, error_code: quinn::VarInt, reason: &[u8]) {
        self.endpoint.close(error_code, reason);
    }

    fn make_server_endpoint(
        bind_addr: std::net::SocketAddr,
        cert: &rcgen::Certificate,
        options: &Option<NetworkServerInitOptions>,
    ) -> anyhow::Result<(Self, Vec<u8>)> {
        let (endpoint, cert) = make_server_endpoint(bind_addr, cert, options)?;
        Ok((
            Self {
                endpoint: endpoint,
                is_server: true,
                allows_05_rtt: if options.is_none() || !options.as_ref().unwrap().disallow_05_rtt {
                    true
                } else {
                    false
                },
            },
            cert,
        ))
    }

    fn make_client_endpoint(
        bind_addr: std::net::SocketAddr,
        server_certs: &[&[u8]],
        options: &Option<NetworkClientInitOptions>,
    ) -> anyhow::Result<Self> {
        let res = make_client_endpoint(bind_addr, server_certs, options)?;
        Ok(Self {
            endpoint: res,
            is_server: false,
            allows_05_rtt: false,
        })
    }

    async fn accept(&self) -> Option<QuinnNetworkConnectingWrapper> {
        let res = self.endpoint.accept().await;
        match res {
            Some(con) => Some(QuinnNetworkConnectingWrapper {
                connecting: con,
                allow_05_rrt: self.is_server && self.allows_05_rtt,
            }),
            None => None,
        }
    }

    fn sock_addr(&self) -> anyhow::Result<SocketAddr> {
        Ok(self.endpoint.local_addr()?)
    }
}

pub type QuinnNetwork =
    Network<QuinnEndpointWrapper, QuinnNetworkConnectionWrapper, QuinnNetworkConnectingWrapper>;
