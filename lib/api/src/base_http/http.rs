use async_trait::async_trait;
use base_io_traits::http_traits::HttpClientInterface;
use bytes::Bytes;

#[derive(Debug)]
pub struct HttpClient {}

impl HttpClient {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl HttpClientInterface for HttpClient {
    async fn download_text(&self, _url: &str) -> anyhow::Result<String> {
        todo!()
    }

    async fn download_binary(&self, _url: &str) -> anyhow::Result<Bytes> {
        todo!()
    }
}
