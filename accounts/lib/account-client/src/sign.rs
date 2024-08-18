use accounts_shared::{
    account_server::sign::SignResponse,
    client::{
        account_data::AccountDataForClient, machine_id::machine_uid, sign::prepare_sign_request,
    },
};
use anyhow::anyhow;
use thiserror::Error;
use x509_parser::oid_registry::asn1_rs::FromDer;

use crate::{
    errors::{FsLikeError, HttpLikeError},
    interface::Io,
    safe_interface::{IoSafe, SafeIo},
};

/// The result of a [`sign`] request.
#[derive(Error, Debug)]
pub enum SignResult {
    /// Session was invalid, must login again.
    #[error("The session was not valid anymore.")]
    SessionWasInvalid,
    /// A file system like error occurred.
    /// This usually means the user was not yet logged in.
    #[error("{0}")]
    FsLikeError(FsLikeError),
    /// A http like error occurred.
    #[error("{0}")]
    HttpLikeError(HttpLikeError),
    /// Errors that are not handled explicitly.
    #[error("Signing failed: {0}")]
    Other(anyhow::Error),
}

impl From<HttpLikeError> for SignResult {
    fn from(value: HttpLikeError) -> Self {
        Self::HttpLikeError(value)
    }
}

impl From<FsLikeError> for SignResult {
    fn from(value: FsLikeError) -> Self {
        Self::FsLikeError(value)
    }
}

/// The sign data contains the signed certificate
/// by the account server, which the client can send
/// to a game server to proof account relationship.
#[derive(Debug, Clone)]
pub struct SignData {
    /// Certificate that was signed by the account server to proof that
    /// the client owns the account.
    /// The cert is in der format.
    pub certificate_der: Vec<u8>,
    /// The account data for this session.
    pub session_key_pair: AccountDataForClient,
}

/// Sign an existing session on the account server.
/// The account server will respond with a certificate,
/// that can be used to verify account ownership on game servers.  
/// __IMPORTANT__: Never share this certificate with anyone.
/// Best is to not even save it to disk, re-sign instead.
///
/// # Errors
///
/// If an error occurs this usually means that the session is not valid anymore.
pub async fn sign(io: &dyn Io) -> anyhow::Result<SignData, SignResult> {
    sign_impl(io.into()).await
}

async fn sign_impl(io: IoSafe<'_>) -> anyhow::Result<SignData, SignResult> {
    // read session's key-pair
    let key_pair = io.read_serialized_session_key_pair().await?;

    let hashed_hw_id = machine_uid().map_err(SignResult::Other)?;

    // do the sign request using the above private key
    let msg = prepare_sign_request(hashed_hw_id, &key_pair.private_key, key_pair.public_key);
    let sign_res = io.request_sign(msg).await?;
    let certificate = match sign_res {
        SignResponse::Success { cert_der } => {
            x509_parser::certificate::X509Certificate::from_der(&cert_der)
                .is_ok()
                .then_some(cert_der)
        }
    };

    certificate.map_or_else(
        || {
            Err(SignResult::Other(anyhow!(
                "the certificate is not in a valid der format"
            )))
        },
        |certificate| {
            Ok(SignData {
                certificate_der: certificate,
                session_key_pair: key_pair,
            })
        },
    )
}
