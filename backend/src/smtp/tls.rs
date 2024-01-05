use rcgen::{Certificate, CertificateParams, DistinguishedName, DnType};
use std::{io::BufReader, sync::Arc};
use tokio::fs;
use tokio_rustls::{
    rustls::{
        self,
        pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer},
    },
    TlsAcceptor,
};
use tracing::info;

use crate::error::Result;

const CERT_PATH: &str = "cert.pem";
const KEY_PATH: &str = "key.pem";

async fn load_certs<'a>() -> Option<Vec<CertificateDer<'a>>> {
    let pem_bytes = fs::read(CERT_PATH).await.ok()?;
    let mut reader = BufReader::new(&pem_bytes[..]);
    let certs: Vec<_> = rustls_pemfile::certs(&mut reader)
        .filter_map(|c| c.ok().map(|der| CertificateDer::from(der.to_vec())))
        .collect();

    if certs.is_empty() {
        None
    } else {
        info!("Loaded certificate {CERT_PATH}");

        Some(certs)
    }
}

async fn load_key<'a>() -> Option<PrivatePkcs8KeyDer<'a>> {
    let pem_bytes = fs::read(KEY_PATH).await.ok()?;
    let mut reader = BufReader::new(&pem_bytes[..]);
    let der = rustls_pemfile::private_key(&mut reader).ok()??;

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

            let full_cert = Certificate::from_params(cert_params)?;
            let cert_pem = full_cert.serialize_pem()?;

            fs::write(CERT_PATH, cert_pem).await?;
            fs::write(KEY_PATH, full_cert.serialize_private_key_pem()).await?;

            let cert = CertificateDer::from(full_cert.serialize_der()?);
            let key = PrivatePkcs8KeyDer::from(full_cert.serialize_private_key_der());

            (vec![cert], key)
        }
    };

    let config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, PrivateKeyDer::Pkcs8(key))?;

    info!("TLS configuration loaded");

    Ok(TlsAcceptor::from(Arc::new(config)))
}
