use axum::async_trait;
use base_io_traits::http_traits::HttpClientInterface;
use bytes::Bytes;

#[derive(Debug, Clone)]
pub struct HttpClient {
    client: reqwest::Client,
}

impl HttpClient {
    pub fn new() -> Self {
        Self {
            client: reqwest::ClientBuilder::new().build().unwrap(),
        }
    }
}

#[async_trait]
impl HttpClientInterface for HttpClient {
    async fn download_text(&self, url: &str) -> anyhow::Result<String> {
        let res = self.client.get(url).send().await?;
        Ok(res.text().await?)
    }

    async fn download_binary(&self, url: &str) -> anyhow::Result<Bytes> {
        let res = self.client.get(url).send().await?;
        Ok(res.bytes().await?)
    }
}
