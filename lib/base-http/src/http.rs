use std::{str::FromStr, sync::Arc};

use async_trait::async_trait;
use base::hash::generate_hash_for;
use base::hash::Hash;
use base_io_traits::http_traits::HttpError;
use base_io_traits::http_traits::{HttpClientInterface, HttpHeaderValue};
use bytes::Bytes;
use reqwest::header::CONTENT_TYPE;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use tokio::sync::Mutex;
use url::Url;

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
    async fn download_text(&self, url: Url) -> anyhow::Result<String> {
        anyhow::ensure!(url.scheme() == "https", "url must be https");
        let res = self.client.get(url).send().await?;
        Ok(res.text().await?)
    }

    async fn download_binary(&self, url: Url, hash: &Hash) -> anyhow::Result<Bytes> {
        anyhow::ensure!(
            url.scheme() == "https" || url.scheme() == "http",
            "url must be http or https"
        );
        let res = self.client.get(url.clone()).send().await?.bytes().await?;

        anyhow::ensure!(
            generate_hash_for(&res).eq(hash),
            format!("file hash mismatched for {url}")
        );

        Ok(res)
    }

    async fn post_json(&self, url: Url, data: Vec<u8>) -> anyhow::Result<Vec<u8>, HttpError> {
        if url.scheme() != "https" {
            return Err(HttpError::Other("url must be http or https".to_string()));
        };

        let res = self
            .client
            .post(url)
            .header(CONTENT_TYPE, HeaderValue::from_static("application/json"))
            .body(data)
            .send()
            .await
            .map_err(|err| {
                if err.is_request() {
                    HttpError::Request
                } else if err.is_status() {
                    HttpError::Status(err.status().unwrap().as_u16())
                } else {
                    HttpError::Other(err.to_string())
                }
            })?;
        Ok(res
            .bytes()
            .await
            .map_err(|err| HttpError::Other(err.to_string()))?
            .to_vec())
    }

    async fn custom_request(
        &self,
        url: Url,
        headers: Vec<HttpHeaderValue>,
    ) -> anyhow::Result<Bytes> {
        anyhow::ensure!(url.scheme() == "https", "url must be https");
        let req = self.client.get(url);

        let mut http_headers = HeaderMap::default();
        for req_header in headers {
            match req_header {
                HttpHeaderValue::String { name, value } => {
                    http_headers
                        .append(HeaderName::from_str(&name)?, HeaderValue::from_str(&value)?);
                }
                HttpHeaderValue::Int { name, value } => {
                    http_headers.append(
                        HeaderName::from_str(&name)?,
                        HeaderValue::from_str(&format!("{value}"))?,
                    );
                }
            }
        }

        let req = req.headers(http_headers);
        let res = req.send().await?.bytes().await?;

        Ok(res)
    }
}

/// a wrapper around [`HttpClient`] that only initializes the http client once it's used (and also initializes it async)
#[derive(Debug, Clone)]
pub struct LazyHttpClient {
    client: Arc<Mutex<Option<HttpClient>>>,
}

impl LazyHttpClient {
    pub fn new() -> Self {
        Self {
            client: Default::default(),
        }
    }

    // get a clone of the underlaying client, if not initialized yet this function will do so.
    pub async fn get_client(&self) -> HttpClient {
        let mut client = self.client.lock().await;
        if client.is_none() {
            *client = Some(HttpClient::new());
        }
        client.clone().unwrap()
    }
}

#[async_trait]
impl HttpClientInterface for LazyHttpClient {
    async fn download_text(&self, url: Url) -> anyhow::Result<String> {
        self.get_client().await.download_text(url).await
    }

    async fn download_binary(&self, url: Url, hash: &Hash) -> anyhow::Result<Bytes> {
        self.get_client().await.download_binary(url, hash).await
    }

    async fn post_json(&self, url: Url, data: Vec<u8>) -> anyhow::Result<Vec<u8>, HttpError> {
        self.get_client().await.post_json(url, data).await
    }

    async fn custom_request(
        &self,
        url: Url,
        headers: Vec<HttpHeaderValue>,
    ) -> anyhow::Result<Bytes> {
        self.get_client().await.custom_request(url, headers).await
    }
}

#[cfg(test)]
mod test {
    use base::benchmark::Benchmark;

    use super::{HttpClient, LazyHttpClient};

    #[test]
    fn http_create_bench() {
        let benchmark = Benchmark::new(true);
        let client = HttpClient::new();
        benchmark.bench("client 1");
        let client2 = HttpClient::new();
        benchmark.bench("client 2");
        drop(client);
        drop(client2);
        let client = LazyHttpClient::new();
        benchmark.bench("client 1 lazy");
        let client2 = LazyHttpClient::new();
        benchmark.bench("client 2 lazy");
        drop(client);
        drop(client2);
    }
}
