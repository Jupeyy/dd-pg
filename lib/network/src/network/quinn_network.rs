use std::{future::Future, pin::Pin};

use super::{
    network::{
        Network, NetworkConnectionInterface, NetworkConnectionRecvStreamInterface,
        NetworkConnectionSendStreamInterface, NetworkEndpointInterface,
    },
    quinnminimal::{make_client_endpoint, make_server_endpoint},
};

#[derive(Clone)]
pub struct QuinnNetworkConnectionWrapper {
    con: quinn::Connection,
}

pub struct QuinnNetworkSendStreamWrapper {
    stream: quinn::SendStream,
}

#[async_trait::async_trait]
impl NetworkConnectionSendStreamInterface for QuinnNetworkSendStreamWrapper {
    async fn write(&mut self, buf: &[u8]) -> anyhow::Result<usize> {
        let res = self.stream.write(buf).await?;
        Ok(res)
    }
    async fn finish(&mut self) -> anyhow::Result<()> {
        let res = self.stream.finish().await?;
        Ok(res)
    }
}

pub struct QuinnNetworkRecvStreamWrapper {
    stream: quinn::RecvStream,
}

#[async_trait::async_trait]
impl NetworkConnectionRecvStreamInterface for QuinnNetworkRecvStreamWrapper {
    async fn read_to_end(&mut self, max_size: usize) -> Result<Vec<u8>, String> {
        let res = self.stream.read_to_end(max_size).await;
        match res {
            Ok(res) => Ok(res),
            Err(err) => Err(err.to_string()),
        }
    }
}

#[async_trait::async_trait]
impl NetworkConnectionInterface<QuinnNetworkSendStreamWrapper, QuinnNetworkRecvStreamWrapper>
    for QuinnNetworkConnectionWrapper
{
    fn send_datagram(&self, data: bytes::Bytes) -> anyhow::Result<()> {
        let res = self.con.send_datagram(data)?;
        Ok(res)
    }

    fn close(&self, error_code: quinn::VarInt, reason: &[u8]) {
        self.con.close(error_code, reason)
    }

    async fn read_datagram(&self) -> Result<Vec<u8>, String> {
        let res = self.con.read_datagram().await;
        match res {
            Ok(res) => Ok(res.to_vec()),
            Err(err) => Err(err.to_string()),
        }
    }

    async fn accept_bi(
        &self,
    ) -> Result<(QuinnNetworkSendStreamWrapper, QuinnNetworkRecvStreamWrapper), String> {
        let res = self.con.accept_bi().await;
        match res {
            Ok((send, recv)) => Ok((
                QuinnNetworkSendStreamWrapper { stream: send },
                QuinnNetworkRecvStreamWrapper { stream: recv },
            )),
            Err(err) => Err(err.to_string()),
        }
    }

    async fn open_bi(
        &self,
    ) -> Result<(QuinnNetworkSendStreamWrapper, QuinnNetworkRecvStreamWrapper), String> {
        let res = self.con.open_bi().await;
        match res {
            Ok((send, recv)) => Ok((
                QuinnNetworkSendStreamWrapper { stream: send },
                QuinnNetworkRecvStreamWrapper { stream: recv },
            )),
            Err(err) => Err(err.to_string()),
        }
    }
}

pub struct QuinnNetworkConnectingWrapper {
    connecting: quinn::Connecting,
}

impl Future for QuinnNetworkConnectingWrapper {
    type Output = Result<QuinnNetworkConnectionWrapper, String>;

    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let con = Pin::new(&mut self.connecting).poll(cx);
        let res = con.map(|f| match f {
            Ok(connection) => Ok(QuinnNetworkConnectionWrapper { con: connection }),
            Err(err) => Err(err.to_string()),
        });
        return res;
    }
}

#[derive(Clone)]
pub struct QuinnEndpointWrapper {
    endpoint: quinn::Endpoint,
}

#[async_trait::async_trait]
impl NetworkEndpointInterface<QuinnNetworkConnectingWrapper> for QuinnEndpointWrapper {
    fn connect(
        &self,
        addr: std::net::SocketAddr,
        server_name: &str,
    ) -> anyhow::Result<QuinnNetworkConnectingWrapper> {
        let res = self.endpoint.connect(addr, server_name)?;
        Ok(QuinnNetworkConnectingWrapper { connecting: res })
    }

    fn close(&self, error_code: quinn::VarInt, reason: &[u8]) {
        self.close(error_code, reason);
    }

    fn make_server_endpoint(
        bind_addr: std::net::SocketAddr,
        cert: &rcgen::Certificate,
    ) -> anyhow::Result<(Self, Vec<u8>)> {
        let (endpoint, cert) = make_server_endpoint(bind_addr, cert)?;
        Ok((Self { endpoint: endpoint }, cert))
    }

    fn make_client_endpoint(
        bind_addr: std::net::SocketAddr,
        server_certs: &[&[u8]],
    ) -> anyhow::Result<Self> {
        let res = make_client_endpoint(bind_addr, server_certs)?;
        Ok(Self { endpoint: res })
    }

    async fn accept(&self) -> Option<QuinnNetworkConnectingWrapper> {
        let res = self.endpoint.accept().await;
        match res {
            Some(con) => Some(QuinnNetworkConnectingWrapper { connecting: con }),
            None => None,
        }
    }
}

pub type QuinnNetwork = Network<
    QuinnEndpointWrapper,
    QuinnNetworkConnectionWrapper,
    QuinnNetworkConnectingWrapper,
    QuinnNetworkSendStreamWrapper,
    QuinnNetworkRecvStreamWrapper,
>;
