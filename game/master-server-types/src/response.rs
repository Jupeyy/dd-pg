use std::borrow::Cow;

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct RegisterError {
    #[serde(skip)]
    pub is_unsupported_media_type: bool,
    pub message: Cow<'static, str>,
}

impl RegisterError {
    pub fn new(s: String) -> RegisterError {
        RegisterError {
            is_unsupported_media_type: false,
            message: Cow::Owned(s),
        }
    }
    pub fn unsupported_media_type() -> RegisterError {
        RegisterError {
            is_unsupported_media_type: true,
            message: Cow::Borrowed("The request's Content-Type is not supported"),
        }
    }
    pub fn status(&self) -> http::StatusCode {
        use http::StatusCode;
        if !self.is_unsupported_media_type {
            StatusCode::BAD_REQUEST
        } else {
            StatusCode::UNSUPPORTED_MEDIA_TYPE
        }
    }
}

impl From<&'static str> for RegisterError {
    fn from(s: &'static str) -> RegisterError {
        RegisterError {
            is_unsupported_media_type: false,
            message: Cow::Borrowed(s),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "status")]
pub enum RegisterResponse {
    Success,
    NeedChallenge,
    NeedInfo,
    Error(RegisterError),
}
