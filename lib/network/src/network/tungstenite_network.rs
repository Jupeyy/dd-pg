use std::{
    collections::VecDeque,
    future::Future,
    net::SocketAddr,
    pin::{pin, Pin},
    sync::Arc,
};

use anyhow::anyhow;
use pool::mt_datatypes::PoolVec;
use tokio::{
    net::{TcpListener, TcpStream},
    sync::Mutex,
    task::JoinHandle,
};
use tokio_tungstenite::{tungstenite::Message, MaybeTlsStream, WebSocketStream};

use super::network::{
    Network, NetworkClientInitOptions, NetworkConnectingInterface, NetworkConnectionInterface,
    NetworkEndpointInterface, NetworkInOrderChannel, NetworkServerInitOptions,
};

use pollster::FutureExt as _;

use futures_util::{
    sink::SinkExt,
    stream::{SplitSink, SplitStream},
    StreamExt,
};

enum ConnectionType {
    Client(
        (
            Mutex<SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>>,
            Mutex<SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>>,
        ),
    ),
    Server(
        (
            Mutex<SplitSink<WebSocketStream<TcpStream>, Message>>,
            Mutex<SplitStream<WebSocketStream<TcpStream>>>,
        ),
    ),
}

#[derive(Clone)]
pub struct TungsteniteNetworkConnectionWrapper {
    con: Arc<ConnectionType>,
    in_order_packets: Arc<tokio::sync::Mutex<VecDeque<PoolVec<u8>>>>,
}

#[async_trait::async_trait]
impl NetworkConnectionInterface for TungsteniteNetworkConnectionWrapper {
    async fn send_unreliable_unordered(&self, data: PoolVec<u8>) -> anyhow::Result<()> {
        // only reliable packets in websocket,
        // they might still come out of order because of the
        // overlaying network implementation
        self.send_unordered_reliable(data).await
    }

    async fn read_unreliable_unordered(&self) -> anyhow::Result<Vec<u8>> {
        todo!()
    }

    async fn send_unordered_reliable(&self, data: PoolVec<u8>) -> anyhow::Result<()> {
        match self.con.as_ref() {
            ConnectionType::Client((send, _)) => {
                send.lock()
                    .block_on()
                    .send(Message::Binary(data.take()))
                    .block_on()
                    .unwrap();
                Ok(())
            }
            ConnectionType::Server((send, _)) => {
                send.lock()
                    .block_on()
                    .send(Message::Binary(data.take()))
                    .block_on()
                    .unwrap();
                Ok(())
            }
        }
    }

    async fn read_unordered_reliable<
        F: FnOnce(anyhow::Result<Vec<u8>>) -> JoinHandle<()> + Send + 'static,
    >(
        &self,
        _on_data: F,
    ) -> anyhow::Result<()> {
        todo!()
    }

    async fn push_ordered_reliable_packet_in_order(
        &self,
        data: PoolVec<u8>,
        _channel: NetworkInOrderChannel,
    ) {
        self.in_order_packets.lock().await.push_back(data);
    }

    async fn send_one_ordered_reliable(
        &self,
        _channel: NetworkInOrderChannel,
    ) -> anyhow::Result<()> {
        let mut packet_guard = self.in_order_packets.lock().await;
        let packet_res = packet_guard.pop_front();
        if let Some(packet) = packet_res {
            // lock guarantees order here
            self.send_unordered_reliable(packet).await
        } else {
            Err(anyhow!("No packet was queued."))
        }
    }

    async fn read_ordered_reliable<
        F: FnOnce(anyhow::Result<Vec<u8>>) -> JoinHandle<()> + Send + 'static,
    >(
        &self,
        _on_data: F,
    ) -> anyhow::Result<()> {
        todo!()
    }

    fn close(&self, _error_code: quinn::VarInt, _reason: &[u8]) {
        match self.con.as_ref() {
            ConnectionType::Client((send, _)) => {
                send.lock().block_on().close().block_on().unwrap();
            }
            ConnectionType::Server((send, _)) => {
                send.lock().block_on().close().block_on().unwrap();
            }
        }
    }
}

enum ConnectingTypes {
    Client(std::future::Ready<WebSocketStream<MaybeTlsStream<TcpStream>>>),
    Server(
        (
            Pin<
                Box<
                    dyn Future<
                        Output = Result<
                            WebSocketStream<TcpStream>,
                            tokio_tungstenite::tungstenite::error::Error,
                        >,
                    >,
                >,
            >,
            SocketAddr,
        ),
    ),
}

pub struct TungsteniteNetworkConnectingWrapper {
    connecting: ConnectingTypes,
}

impl Future for TungsteniteNetworkConnectingWrapper {
    type Output = Result<TungsteniteNetworkConnectionWrapper, String>;

    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        match &mut self.connecting {
            ConnectingTypes::Client(connection) => {
                let ws_stream = pin!(connection).poll(cx);
                ws_stream.map(|f| {
                    let (send, recv) = f.split();
                    Ok(TungsteniteNetworkConnectionWrapper {
                        con: Arc::new(ConnectionType::Client((Mutex::new(send), Mutex::new(recv)))),
                        in_order_packets: Default::default(),
                    })
                })
            }
            ConnectingTypes::Server(connection) => {
                let t = Pin::new(&mut connection.0);
                let ws_stream = t.poll(cx);
                ws_stream.map(|f| match f {
                    Ok(connection) => {
                        let (send, recv) = connection.split();
                        Ok(TungsteniteNetworkConnectionWrapper {
                            con: Arc::new(ConnectionType::Server((
                                Mutex::new(send),
                                Mutex::new(recv),
                            ))),
                            in_order_packets: Default::default(),
                        })
                    }
                    Err(err) => Err(err.to_string()),
                })
            }
        }
    }
}

impl NetworkConnectingInterface<TungsteniteNetworkConnectionWrapper>
    for TungsteniteNetworkConnectingWrapper
{
    fn try_fast_unwrap(self) -> Result<TungsteniteNetworkConnectionWrapper, Self> {
        Err(self)
    }
}

#[derive(Clone)]
pub struct TungsteniteEndpointWrapper {
    endpoint: Arc<TcpListener>,
}

#[async_trait::async_trait]
impl NetworkEndpointInterface<TungsteniteNetworkConnectingWrapper> for TungsteniteEndpointWrapper {
    fn connect(
        &self,
        addr: std::net::SocketAddr,
        _server_name: &str,
    ) -> anyhow::Result<TungsteniteNetworkConnectingWrapper> {
        let (res, _) = tokio_tungstenite::connect_async(&addr.to_string()).block_on()?;
        Ok(TungsteniteNetworkConnectingWrapper {
            connecting: ConnectingTypes::Client(std::future::ready(res)),
        })
    }

    fn close(&self, _error_code: quinn::VarInt, _reason: &[u8]) {
        // TODO: mhh self.endpoint.close();
    }

    fn make_server_endpoint(
        bind_addr: std::net::SocketAddr,
        _cert: &rcgen::Certificate,
        _options: &Option<NetworkServerInitOptions>,
    ) -> anyhow::Result<(Self, Vec<u8>)> {
        let (endpoint, cert) = (TcpListener::bind(&bind_addr).block_on()?, vec![]);
        Ok((
            Self {
                endpoint: Arc::new(endpoint),
            },
            cert,
        ))
    }

    fn make_client_endpoint(
        bind_addr: std::net::SocketAddr,
        _server_certs: &[&[u8]],
        _options: &Option<NetworkClientInitOptions>,
    ) -> anyhow::Result<Self> {
        let res = TcpListener::bind(&bind_addr).block_on()?;
        Ok(Self {
            endpoint: Arc::new(res),
        })
    }

    async fn accept(&self) -> Option<TungsteniteNetworkConnectingWrapper> {
        let res = self.endpoint.accept().await;
        match res {
            Ok(con) => Some(TungsteniteNetworkConnectingWrapper {
                connecting: ConnectingTypes::Server((
                    Box::pin(tokio_tungstenite::accept_async(con.0)),
                    con.1,
                )),
            }),
            Err(_) => None,
        }
    }

    fn sock_addr(&self) -> anyhow::Result<SocketAddr> {
        Ok(self.endpoint.local_addr()?)
    }
}

pub type TungsteniteNetwork = Network<
    TungsteniteEndpointWrapper,
    TungsteniteNetworkConnectionWrapper,
    TungsteniteNetworkConnectingWrapper,
>;
