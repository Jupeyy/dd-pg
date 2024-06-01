use std::sync::Arc;

use accounts_base::{account_server::otp::OtpResponse, client::otp::OtpRequest};
use anyhow::anyhow;
use axum::{response, Json};
use sqlx::MySqlPool;

use crate::{internal_err::InternalErr, shared::Shared};

/// The client requests an one time password
pub fn otp_request(
    shared: Arc<Shared>,
    _pool: MySqlPool,
    Json(data): Json<OtpRequest>,
) -> response::Result<Json<OtpResponse>> {
    Ok(Json(OtpResponse {
        otps: {
            (data.count <= 2).then_some(()).ok_or_else(|| {
                InternalErr((
                    "otp".into(),
                    anyhow!("Only at most 2 one time passwords are allowed in one request"),
                ))
            })?;

            let mut res = Vec::default();
            res.resize_with(data.count as usize, || shared.otps.gen_otp());
            res
        },
    }))
}
