use std::sync::Arc;

use account_client::errors::HttpLikeError;
use anyhow::anyhow;
use async_trait::async_trait;
use base_io_traits::http_traits::{HttpClientInterface, HttpError};
use client_http_fs::http::Http;
use url::Url;

#[derive(Debug)]
pub struct AccountHttp {
    pub(crate) base_url: Url,
    pub(crate) http: Arc<dyn HttpClientInterface>,
}

#[async_trait]
impl Http for AccountHttp {
    fn new(_base_url: Url) -> Self
    where
        Self: Sized,
    {
        panic!("not implemented")
    }
    async fn post_json(&self, url: Url, data: Vec<u8>) -> anyhow::Result<Vec<u8>, HttpLikeError> {
        self.http
            .post_json(url, data)
            .await
            .map_err(|err| match err {
                HttpError::Request => HttpLikeError::Request,
                HttpError::Status(code) => HttpLikeError::Status(code),
                HttpError::Other(err) => HttpLikeError::Other(anyhow!(err)),
            })
    }
    fn base_url(&self) -> Url {
        self.base_url.clone()
    }
}
