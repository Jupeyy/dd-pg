use std::fmt::Debug;

use async_trait::async_trait;
use bytes::Bytes;

#[async_trait]
pub trait HttpClientInterface: Debug + Send + Sync {
    async fn download_text(&self, url: &str) -> anyhow::Result<String>;

    async fn download_binary(&self, url: &str) -> anyhow::Result<Bytes>;
}
