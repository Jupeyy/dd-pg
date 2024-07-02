use std::fmt::Debug;

use async_trait::async_trait;
use base::hash::Hash;
use bytes::Bytes;
use serde::{Deserialize, Serialize};
use thiserror::Error;
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

/// An error that is similar to
/// common http errrors.
/// Used for requests to the account
/// server.
#[derive(Error, Debug, Serialize, Deserialize)]
pub enum HttpError {
    /// The request failed.
    #[error("The request failed to be sent.")]
    Request,
    /// Http status codes.
    #[error("The server responsed with status code {0}")]
    Status(u16),
    /// Other errors
    #[error("{0}")]
    Other(String),
}

#[async_trait]
pub trait HttpClientInterface: Debug + Send + Sync {
    async fn download_text(&self, url: Url) -> anyhow::Result<String>;

    /// Downloads binary data. This only allows reading binary data where the hash is already known
    async fn download_binary(&self, url: Url, hash: &Hash) -> anyhow::Result<Bytes>;

    /// Post a json body and return arbitrary bytes returned as a response.
    async fn post_json(&self, url: Url, data: Vec<u8>) -> anyhow::Result<Vec<u8>, HttpError>;

    async fn custom_request(
        &self,
        url: Url,
        headers: Vec<HttpHeaderValue>,
    ) -> anyhow::Result<Bytes>;
}
