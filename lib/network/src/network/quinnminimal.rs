//! Commonly used code in most examples.

use anyhow::anyhow;
use base::hash::{generate_hash_for, Hash};
use quinn::{
    crypto::rustls::{QuicClientConfig, QuicServerConfig},
    ClientConfig, Endpoint, IdleTimeout, ServerConfig, TransportConfig,
};
use rcgen::CertifiedKey;
use rustls::crypto::CryptoProvider;
use std::{net::SocketAddr, sync::Arc, time::Duration};
use x509_certificate::X509Certificate;

use super::network::{
    NetworkClientCertCheckMode, NetworkClientCertMode, NetworkClientInitOptions,
    NetworkServerCertMode, NetworkServerCertModeResult, NetworkServerInitOptions,
};

/// Constructs a QUIC endpoint configured for use a client only.
///
/// ## Args
///
/// - server_certs: list of trusted certificates.
pub fn make_client_endpoint(
    bind_addr: SocketAddr,
    options: &NetworkClientInitOptions,
) -> anyhow::Result<Endpoint> {
    let client_cfg = configure_client(options)?;
    let mut endpoint = Endpoint::client(bind_addr)?;
    endpoint.set_default_client_config(client_cfg);
    Ok(endpoint)
}

/// Constructs a QUIC endpoint configured to listen for incoming connections on a certain address
/// and port.
///
/// ## Returns
///
/// - a stream of incoming QUIC connections
/// - server certificate serialized into DER format
pub fn make_server_endpoint(
    bind_addr: SocketAddr,
    cert_mode: NetworkServerCertMode,
    options: &NetworkServerInitOptions,
) -> anyhow::Result<(Endpoint, NetworkServerCertModeResult)> {
    let (server_config, server_cert) = configure_server(cert_mode, options)?;
    //eprintln!("{:#?}", server_config);
    let endpoint = Endpoint::server(server_config, bind_addr)?;
    Ok((endpoint, server_cert))
}

pub fn create_certifified_keys() -> CertifiedKey {
    rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap()
}

/// dummy certificate handler
#[derive(Debug)]
struct SkipServerVerification;

impl SkipServerVerification {
    fn new() -> Arc<Self> {
        Arc::new(Self)
    }
}

impl rustls::client::danger::ServerCertVerifier for SkipServerVerification {
    fn verify_server_cert(
        &self,
        _end_entity: &rustls::pki_types::CertificateDer<'_>,
        _intermediates: &[rustls::pki_types::CertificateDer<'_>],
        _server_name: &rustls::pki_types::ServerName<'_>,
        _ocsp_response: &[u8],
        _now: rustls::pki_types::UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        todo!()
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        todo!()
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        todo!()
    }
}

/// hash certificate handler
#[derive(Debug)]
struct CertHashServerVerification {
    hash: Hash,
    provider: Arc<rustls::crypto::CryptoProvider>,
}

impl CertHashServerVerification {
    fn new(provider: Arc<rustls::crypto::CryptoProvider>, hash: Hash) -> Arc<Self> {
        Arc::new(Self { hash, provider })
    }
}

impl rustls::client::danger::ServerCertVerifier for CertHashServerVerification {
    fn verify_server_cert(
        &self,
        end_entity: &rustls::pki_types::CertificateDer<'_>,
        _intermediates: &[rustls::pki_types::CertificateDer<'_>],
        _server_name: &rustls::pki_types::ServerName<'_>,
        _ocsp_response: &[u8],
        _now: rustls::pki_types::UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        let cert = X509Certificate::from_der(end_entity).map_err(|_| {
            rustls::Error::InvalidCertificate(rustls::CertificateError::BadEncoding)
        })?;

        let hash = generate_hash_for(&cert.public_key_data().to_vec());
        if self.hash.eq(&hash) {
            Ok(rustls::client::danger::ServerCertVerified::assertion())
        } else {
            Err(rustls::Error::InvalidCertificate(
                rustls::CertificateError::BadSignature,
            ))
        }
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &rustls::pki_types::CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &rustls::pki_types::CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        self.provider
            .signature_verification_algorithms
            .supported_schemes()
    }
}

/// Builds default quinn client config and trusts given certificates.
///
/// ## Args
///
/// - server_certs: a list of trusted certificates in DER format.
fn configure_client(options: &NetworkClientInitOptions) -> anyhow::Result<ClientConfig> {
    let mut transport_config = TransportConfig::default();
    transport_config
        .max_concurrent_bidi_streams(500u32.into())
        .max_concurrent_uni_streams(500u32.into());
    if options
        .base
        .timeout
        .is_some_and(|timeout| timeout != Duration::ZERO)
    {
        transport_config
            .max_idle_timeout(IdleTimeout::try_from(options.base.timeout.unwrap()).ok());
    }
    let transport = Arc::new(transport_config);

    let (cert, priv_key) = match &options.cert {
        NetworkClientCertMode::FromCertifiedKeyPair { cert } => (
            vec![cert.cert.der().clone()],
            cert.key_pair
                .serialize_der()
                .try_into()
                .map_err(|_| anyhow!("priv key was not a valid to be serialized as der"))?,
        ),
    };

    match options.cert_check {
        NetworkClientCertCheckMode::CheckByCert { cert: server_cert } => {
            let mut certs = rustls::RootCertStore::empty();
            certs.add(server_cert.into())?;

            if CryptoProvider::get_default().is_none() {
                CryptoProvider::install_default(rustls::crypto::ring::default_provider())
                    .map_err(|_| anyhow!("ring crypto provider could not be initialized"))?;
            }
            let provider = Arc::new(rustls::crypto::ring::default_provider());
            let mut client_config = ClientConfig::new(Arc::new(QuicClientConfig::try_from({
                rustls::ClientConfig::builder_with_provider(provider)
                    .with_safe_default_protocol_versions()?
                    .with_root_certificates(certs)
                    .with_client_auth_cert(cert, priv_key)?
            })?));
            client_config.transport_config(transport);
            Ok(client_config)
        }
        NetworkClientCertCheckMode::CheckByPubKeyHash { hash } => {
            let provider = Arc::new(rustls::crypto::ring::default_provider());
            let mut client_config = ClientConfig::new(Arc::new(QuicClientConfig::try_from({
                rustls::ClientConfig::builder_with_provider(provider.clone())
                    .with_safe_default_protocol_versions()?
                    .dangerous()
                    .with_custom_certificate_verifier(CertHashServerVerification::new(
                        provider, *hash,
                    ))
                    .with_client_auth_cert(cert, priv_key)?
            })?));
            client_config.transport_config(transport);
            Ok(client_config)
        }
        NetworkClientCertCheckMode::DisableCheck => {
            let provider = Arc::new(rustls::crypto::ring::default_provider());
            let mut client_config = ClientConfig::new(Arc::new(QuicClientConfig::try_from({
                rustls::ClientConfig::builder_with_provider(provider)
                    .with_safe_default_protocol_versions()?
                    .dangerous()
                    .with_custom_certificate_verifier(SkipServerVerification::new())
                    .with_client_auth_cert(cert, priv_key)?
            })?));
            client_config.transport_config(transport);
            Ok(client_config)
        }
    }
}

#[derive(Debug)]
struct ServerClientCertVerifier(Arc<CryptoProvider>);

impl rustls::server::danger::ClientCertVerifier for ServerClientCertVerifier {
    fn root_hint_subjects(&self) -> &[rustls::DistinguishedName] {
        &[]
    }

    fn verify_client_cert(
        &self,
        end_entity: &rustls::pki_types::CertificateDer<'_>,
        _intermediates: &[rustls::pki_types::CertificateDer<'_>],
        _now: rustls::pki_types::UnixTime,
    ) -> Result<rustls::server::danger::ClientCertVerified, rustls::Error> {
        // just make sure that the cert is x509
        // we don't actually care if the cert is valid, we just want the
        // public key as hash for identification
        X509Certificate::from_der(end_entity)
            .map_err(|_| rustls::Error::InvalidCertificate(rustls::CertificateError::BadEncoding))
            .map(|_| rustls::server::danger::ClientCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &rustls::pki_types::CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(
            message,
            cert,
            dss,
            &self.0.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &rustls::pki_types::CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(
            message,
            cert,
            dss,
            &self.0.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        self.0.signature_verification_algorithms.supported_schemes()
    }
}

/// Returns default server configuration along with its certificate.
fn configure_server(
    cert_mode: NetworkServerCertMode,
    options: &NetworkServerInitOptions,
) -> anyhow::Result<(ServerConfig, NetworkServerCertModeResult)> {
    let (priv_key, cert_chain, cert) = match cert_mode {
        NetworkServerCertMode::FromCertifiedKeyPair { cert } => {
            let priv_key = cert.key_pair.serialize_der();
            (
                priv_key
                    .try_into()
                    .map_err(|_| anyhow!("converting cert failed"))?,
                vec![cert
                    .cert
                    .der()
                    .to_vec()
                    .try_into()
                    .map_err(|_| anyhow!("converting cert failed"))?],
                NetworkServerCertModeResult::Cert { cert: cert.cert },
            )
        }
    };

    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let mut server_config = ServerConfig::with_crypto(Arc::new(QuicServerConfig::try_from(
        rustls::ServerConfig::builder_with_provider(provider.clone())
            .with_safe_default_protocol_versions()?
            .with_client_cert_verifier(Arc::new(ServerClientCertVerifier(provider.clone())))
            .with_single_cert(cert_chain, priv_key)?,
    )?));
    let transport = Arc::get_mut(&mut server_config.transport).unwrap();
    transport
        .keep_alive_interval(Some(Duration::from_millis(1000)))
        .max_concurrent_bidi_streams(500u32.into())
        .max_concurrent_uni_streams(500u32.into());

    if options
        .base
        .timeout
        .is_some_and(|timeout| timeout != Duration::ZERO)
    {
        transport.max_idle_timeout(IdleTimeout::try_from(options.base.timeout.unwrap()).ok());
    }

    Ok((server_config, cert))
}

#[allow(unused)]
pub const ALPN_QUIC_HTTP: &[&[u8]] = &[b"hq-29"];
