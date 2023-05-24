use std::{
    future::Future,
    net::SocketAddr,
    pin::{pin, Pin},
    sync::Arc,
    task::Poll,
};

use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::{tungstenite::Message, MaybeTlsStream, WebSocketStream};

use super::network::{
    Network, NetworkConnectionInterface, NetworkConnectionRecvStreamInterface,
    NetworkConnectionSendStreamInterface, NetworkEndpointInterface,
};

use pollster::FutureExt as _;

use futures_util::{
    sink::{Sink, SinkExt},
    stream::{FusedStream, SplitSink, SplitStream, Stream},
    StreamExt,
};

enum ConnectionType {
    Client(WebSocketStream<MaybeTlsStream<TcpStream>>),
    Server(WebSocketStream<TcpStream>),
}

#[derive(Clone)]
pub struct TungsteniteNetworkConnectionWrapper {
    con: Arc<ConnectionType>,
}

enum StreamType {
    Client(SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>),
    Server(SplitSink<WebSocketStream<TcpStream>, Message>),
}

pub struct TungsteniteNetworkSendStreamWrapper {
    stream: StreamType,
}

#[async_trait::async_trait]
impl NetworkConnectionSendStreamInterface for TungsteniteNetworkSendStreamWrapper {
    async fn write(&mut self, buf: &[u8]) -> anyhow::Result<usize> {
        //let res = self.stream.write(buf).await?;
        // Ok(res)
        Ok(0)
    }
    async fn finish(&mut self) -> anyhow::Result<()> {
        //let res = self.stream.finish().await?;
        //Ok(res)
        Ok(())
    }
}

enum RecvStreamType {
    Client(SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>),
    Server(SplitStream<WebSocketStream<TcpStream>>),
}

pub struct TungsteniteNetworkRecvStreamWrapper {
    stream: RecvStreamType,
}

#[async_trait::async_trait]
impl NetworkConnectionRecvStreamInterface for TungsteniteNetworkRecvStreamWrapper {
    async fn read_to_end(&mut self, max_size: usize) -> Result<Vec<u8>, String> {
        /*let res = self.stream.read_to_end(max_size).await;
        match res {
            Ok(res) => Ok(res),
            Err(err) => Err(err.to_string()),
        }*/
        Ok(Vec::new())
    }
}

#[async_trait::async_trait]
impl
    NetworkConnectionInterface<
        TungsteniteNetworkSendStreamWrapper,
        TungsteniteNetworkRecvStreamWrapper,
    > for TungsteniteNetworkConnectionWrapper
{
    fn send_datagram(&self, data: bytes::Bytes) -> anyhow::Result<()> {
        //let res = self.con.send_datagram(data)?;
        //Ok(res)
        Ok(())
    }

    fn close(&self, error_code: quinn::VarInt, reason: &[u8]) {
        //self.con.close(error_code, reason)
    }

    async fn read_datagram(&self) -> Result<Vec<u8>, String> {
        /*let res = self.con.read_datagram().await;
        match res {
            Ok(res) => Ok(res.to_vec()),
            Err(err) => Err(err.to_string()),
        }*/
        Ok(Vec::new())
    }

    async fn accept_bi(
        &self,
    ) -> Result<
        (
            TungsteniteNetworkSendStreamWrapper,
            TungsteniteNetworkRecvStreamWrapper,
        ),
        String,
    > {
        self.open_bi().await
    }

    async fn open_bi(
        &self,
    ) -> Result<
        (
            TungsteniteNetworkSendStreamWrapper,
            TungsteniteNetworkRecvStreamWrapper,
        ),
        String,
    > {
        /*  Ok(match self.con.as_ref() {
            ConnectionType::Client(c) => {
                let (send, recv) = c.split();
                (
                    TungsteniteNetworkSendStreamWrapper {
                        stream: StreamType::Client(send),
                    },
                    TungsteniteNetworkRecvStreamWrapper {
                        stream: RecvStreamType::Client(recv),
                    },
                )
            }
            ConnectionType::Server(c) => {
                let (send, recv) = c.split();
                (
                    TungsteniteNetworkSendStreamWrapper {
                        stream: StreamType::Server(send),
                    },
                    TungsteniteNetworkRecvStreamWrapper {
                        stream: RecvStreamType::Server(recv),
                    },
                )
            }
        })*/
        Err("tes".to_string())
    }
}

enum ConnectingTypes {
    Client(std::future::Ready<WebSocketStream<MaybeTlsStream<TcpStream>>>),
    Server(
        (
            Box<
                dyn Future<
                    Output = Result<
                        WebSocketStream<TcpStream>,
                        tokio_tungstenite::tungstenite::error::Error,
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
        match self.connecting {
            ConnectingTypes::Client(connection) => {
                let ws_stream = pin!(connection).poll(cx);
                ws_stream.map(|f| {
                    Ok(TungsteniteNetworkConnectionWrapper {
                        con: Arc::new(ConnectionType::Client(f)),
                    })
                })
            }
            ConnectingTypes::Server(connection) => {
                let connection = connection.0;
                let ws_stream = pin!(*connection).poll(cx);
                ws_stream.map(|f| match f {
                    Ok(connection) => Ok(TungsteniteNetworkConnectionWrapper {
                        con: Arc::new(ConnectionType::Server(connection)),
                    }),
                    Err(err) => Err(err.to_string()),
                })
            }
        }
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
        server_name: &str,
    ) -> anyhow::Result<TungsteniteNetworkConnectingWrapper> {
        let (res, _) = tokio_tungstenite::connect_async(&addr.to_string()).block_on()?;
        Ok(TungsteniteNetworkConnectingWrapper {
            connecting: ConnectingTypes::Client(std::future::ready(res)),
        })
    }

    fn close(&self, error_code: quinn::VarInt, reason: &[u8]) {
        self.close(error_code, reason);
    }

    fn make_server_endpoint(
        bind_addr: std::net::SocketAddr,
        cert: &rcgen::Certificate,
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
        server_certs: &[&[u8]],
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
                    Box::new(tokio_tungstenite::accept_async(con.0)),
                    con.1,
                )),
            }),
            Err(_) => None,
        }
    }
}

pub type TungsteniteNetwork = Network<
    TungsteniteEndpointWrapper,
    TungsteniteNetworkConnectionWrapper,
    TungsteniteNetworkConnectingWrapper,
    TungsteniteNetworkSendStreamWrapper,
    TungsteniteNetworkRecvStreamWrapper,
>;
