use ed25519_dalek::{
    pkcs8::{spki::der::pem::LineEnding, EncodePrivateKey},
    SigningKey,
};
use rcgen::{CertificateParams, KeyPair, PKCS_ED25519};

pub use rcgen::CertifiedKey;

/// Generates a self signed certificate and key-pair as [`CertifiedKey`]
/// from a ed25519 private key.
pub fn generate_self_signed(private_key: &SigningKey) -> anyhow::Result<CertifiedKey> {
    let key = private_key.to_pkcs8_pem(LineEnding::LF)?;
    let key_pair = KeyPair::from_pkcs8_pem_and_sign_algo(&key, &PKCS_ED25519)?;
    let cert = CertificateParams::new(vec!["localhost".into()])?.self_signed(&key_pair)?;
    Ok(CertifiedKey { cert, key_pair })
}
