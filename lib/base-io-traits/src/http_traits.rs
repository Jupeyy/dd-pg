use std::fmt::Debug;

use async_trait::async_trait;
use base::hash::Hash;
use bytes::Bytes;
use url::Url;

pub enum HttpHeaderValue {
    String { name: String, value: String },
    Int { name: String, value: i32 },
}

impl From<(&str, &str)> for HttpHeaderValue {
    fn from(value: (&str, &str)) -> Self {
        Self::String {
            name: value.0.to_string(),
            value: value.1.to_string(),
        }
    }
}

impl From<(&str, i32)> for HttpHeaderValue {
    fn from(value: (&str, i32)) -> Self {
        Self::Int {
            name: value.0.to_string(),
            value: value.1,
        }
    }
}

#[async_trait]
pub trait HttpClientInterface: Debug + Send + Sync {
    async fn download_text(&self, url: Url) -> anyhow::Result<String>;

    /// Downloads binary data. This only allows reading binary data where the hash is already known
    async fn download_binary(&self, url: Url, hash: &Hash) -> anyhow::Result<Bytes>;

    async fn custom_request(
        &self,
        url: Url,
        headers: Vec<HttpHeaderValue>,
    ) -> anyhow::Result<Bytes>;
}
