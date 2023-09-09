//! Commonly used code in most examples.

use quinn::{ClientConfig, Endpoint, IdleTimeout, ServerConfig, TransportConfig};
use rcgen::Certificate;
use std::{net::SocketAddr, sync::Arc, time::Duration};

use super::network::{NetworkClientInitOptions, NetworkServerInitOptions};

/// Constructs a QUIC endpoint configured for use a client only.
///
/// ## Args
///
/// - server_certs: list of trusted certificates.
pub fn make_client_endpoint(
    bind_addr: SocketAddr,
    server_certs: &[&[u8]],
    options: &NetworkClientInitOptions,
) -> anyhow::Result<Endpoint> {
    let client_cfg = configure_client(server_certs, options)?;
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
    cert: &Certificate,
    options: &NetworkServerInitOptions,
) -> anyhow::Result<(Endpoint, Vec<u8>)> {
    let (server_config, server_cert) = configure_server(cert, options)?;
    //eprintln!("{:#?}", server_config);
    let endpoint = Endpoint::server(server_config, bind_addr)?;
    Ok((endpoint, server_cert))
}

pub fn create_certificate() -> Certificate {
    rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap()
}

// dummy certificate handler
struct SkipServerVerification;

impl SkipServerVerification {
    fn new() -> Arc<Self> {
        Arc::new(Self)
    }
}

impl rustls::client::ServerCertVerifier for SkipServerVerification {
    fn verify_server_cert(
        &self,
        _end_entity: &rustls::Certificate,
        _intermediates: &[rustls::Certificate],
        _server_name: &rustls::ServerName,
        _scts: &mut dyn Iterator<Item = &[u8]>,
        _ocsp_response: &[u8],
        _now: std::time::SystemTime,
    ) -> Result<rustls::client::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::ServerCertVerified::assertion())
    }
}

/// Builds default quinn client config and trusts given certificates.
///
/// ## Args
///
/// - server_certs: a list of trusted certificates in DER format.
fn configure_client(
    server_certs: &[&[u8]],
    options: &NetworkClientInitOptions,
) -> anyhow::Result<ClientConfig> {
    let mut certs = rustls::RootCertStore::empty();
    for cert in server_certs {
        certs.add(&rustls::Certificate(cert.to_vec()))?;
    }

    let ignore_cert = options.skip_cert_check.unwrap_or(false);
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
    if ignore_cert {
        let mut client_config = ClientConfig::new(Arc::new(
            rustls::ClientConfig::builder()
                .with_safe_defaults()
                .with_custom_certificate_verifier(SkipServerVerification::new())
                .with_no_client_auth(),
        ));
        client_config.transport_config(transport);
        Ok(client_config)
    } else {
        let mut client_config = ClientConfig::with_root_certificates(certs);
        client_config.transport_config(transport);
        Ok(client_config)
    }
}

/// Returns default server configuration along with its certificate.
fn configure_server(
    cert: &Certificate,
    options: &NetworkServerInitOptions,
) -> anyhow::Result<(ServerConfig, Vec<u8>)> {
    let cert_der = cert.serialize_der().unwrap();
    let priv_key = cert.serialize_private_key_der();
    let priv_key = rustls::PrivateKey(priv_key);
    let cert_chain = vec![rustls::Certificate(cert_der.clone())];

    let mut server_config = ServerConfig::with_single_cert(cert_chain, priv_key)?;
    Arc::get_mut(&mut server_config.transport)
        .unwrap()
        .keep_alive_interval(Some(Duration::from_millis(1000)))
        .max_concurrent_bidi_streams(500u32.into())
        .max_concurrent_uni_streams(500u32.into());

    if options
        .base
        .timeout
        .is_some_and(|timeout| timeout != Duration::ZERO)
    {
        Arc::get_mut(&mut server_config.transport)
            .unwrap()
            .max_idle_timeout(IdleTimeout::try_from(options.base.timeout.unwrap()).ok());
    }

    Ok((server_config, cert_der))
}

#[allow(unused)]
pub const ALPN_QUIC_HTTP: &[&[u8]] = &[b"hq-29"];
