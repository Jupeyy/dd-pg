use async_trait::async_trait;
use base::hash::Hash;
use base_io_traits::http_traits::{HttpClientInterface, HttpHeaderValue};
use bytes::Bytes;
use url::Url;

#[derive(Debug)]
pub struct HttpClient {}

impl HttpClient {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl HttpClientInterface for HttpClient {
    async fn download_text(&self, _url: Url) -> anyhow::Result<String> {
        todo!()
    }

    async fn download_binary(&self, _url: Url, _hash: &Hash) -> anyhow::Result<Bytes> {
        todo!()
    }

    async fn custom_request(
        &self,
        _url: Url,
        _headers: Vec<HttpHeaderValue>,
    ) -> anyhow::Result<Bytes> {
        todo!()
    }
}
