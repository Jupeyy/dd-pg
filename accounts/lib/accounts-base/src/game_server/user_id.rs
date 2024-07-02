use der::{Decode, Encode};
use ed25519_dalek::Verifier;
use p256::ecdsa::{Signature, VerifyingKey};
use serde::{Deserialize, Serialize};

use crate::account_server::{account_id::AccountId, cert_account_ext::AccountCertExt};

/// A type that represents an user id
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserId {
    /// The optional account id.
    /// If this is `Some` the game server is garantueed
    /// that the user has the account to this account id.
    pub account_id: Option<AccountId>,
    /// As fallback if no account id was given,
    /// the public key (hash/fingerprint) is used to identify the user.
    pub public_key: [u8; 32],
}

/// Get the user id from a public key send by a client.
///
/// This function pre-assumes that the certificate is a valid x509 certificate
/// and contains a subject's public key info that can be converted to a
/// fingerprint.
///
/// # Panics
/// Panics, if the cert is not a valid x509 certificate.
/// This should already be checked in the TLS handshake (or similar).
pub fn user_id_from_pub_key(account_server_public_key: &VerifyingKey, cert_der: Vec<u8>) -> UserId {
    let mut account_id = None;

    let Ok(cert) = x509_cert::Certificate::from_der(&cert_der) else {
        panic!("not a valid x509 certificate.")
    };
    let public_key = cert
        .tbs_certificate
        .subject_public_key_info
        .fingerprint_bytes()
        .unwrap_or_default();

    if let Ok(der) = cert.tbs_certificate.to_der() {
        let sig_res = Signature::from_der(cert.signature.raw_bytes());
        if let Ok(signature) = sig_res {
            let verify_res = account_server_public_key.verify(&der, &signature);
            if verify_res.is_ok() {
                if let Ok(Some((_, account_data))) = cert.tbs_certificate.get::<AccountCertExt>() {
                    account_id = Some(account_data.data.account_id);
                }
            }
        }
    }

    UserId {
        account_id,
        public_key,
    }
}
