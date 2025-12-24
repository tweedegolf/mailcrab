use rcgen::{CertificateParams, DistinguishedName, DnType, KeyPair};
use rustls_pki_types::pem::PemObject;
use std::{io::BufReader, sync::Arc};
use tokio::fs;
use tokio_rustls::{
    TlsAcceptor,
    rustls::{
        self,
        pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer},
    },
};
use tracing::info;

use crate::error::Result;

const CERT_PATH: &str = "cert.pem";
const KEY_PATH: &str = "key.pem";

async fn load_certs<'a>() -> Option<Vec<CertificateDer<'a>>> {
    let pem_bytes = fs::read(CERT_PATH).await.ok()?;
    let mut reader = BufReader::new(&pem_bytes[..]);
    let certs: Vec<_> = CertificateDer::pem_reader_iter(&mut reader)
        .filter_map(|c| c.ok().map(|der| CertificateDer::from(der.to_vec())))
        .collect();

    if certs.is_empty() {
        None
    } else {
        info!(
            "Certificate loaded from disk:\n{}",
            String::from_utf8_lossy(&pem_bytes)
        );

        Some(certs)
    }
}

async fn load_key<'a>() -> Option<PrivatePkcs8KeyDer<'a>> {
    let pem_bytes = fs::read(KEY_PATH).await.ok()?;
    let mut reader = BufReader::new(&pem_bytes[..]);
    let der = PrivateKeyDer::from_pem_reader(&mut reader).ok()?;

    info!("Loaded key {KEY_PATH}");

    Some(PrivatePkcs8KeyDer::from(der.secret_der().to_vec()))
}

/// read or generate a certioficate + key for the SMTP server
pub(super) async fn create_tls_acceptor(name: &str) -> Result<TlsAcceptor> {
    let (certs, key) = match (load_certs().await, load_key().await) {
        (Some(cert), Some(key)) => (cert, key),
        _ => {
            info!("Generating self-signed certificate...");

            let mut cert_params = CertificateParams::default();
            let mut dis_name = DistinguishedName::new();
            dis_name.push(DnType::CommonName, name);
            cert_params.distinguished_name = dis_name;

            let key_pair = KeyPair::generate()?;
            let cert = cert_params.self_signed(&key_pair)?;

            fs::write(CERT_PATH, cert.pem()).await?;
            fs::write(KEY_PATH, key_pair.serialize_pem()).await?;

            info!("Certificate generated:\n{}", cert.pem());

            let cert = cert.der().clone();
            let key = PrivatePkcs8KeyDer::from(key_pair.serialize_der());

            (vec![cert], key)
        }
    };

    let config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, PrivateKeyDer::Pkcs8(key))?;

    info!("TLS configuration loaded");

    Ok(TlsAcceptor::from(Arc::new(config)))
}
