use mailin::AuthMechanism;
/// Receives email over SMTP, parses and broadcasts messages over an internal queue
use mailin_embedded::{Server, SslConfig};
use rcgen::{Certificate, CertificateParams, DistinguishedName, DnType};
use std::{
    fs,
    io::{self},
    net::ToSocketAddrs,
};
use tokio::sync::broadcast::Sender;
use tracing::{event, Level};

use crate::types::MailMessage;

#[derive(Clone, Debug)]
struct MailHandler {
    // internal broadcast queue
    tx: Sender<MailMessage>,

    // incoming message buffer
    buffer: Vec<u8>,
}

impl MailHandler {
    fn create(tx: Sender<MailMessage>) -> Self {
        MailHandler {
            tx,
            buffer: Vec::new(),
        }
    }
}

impl MailHandler {
    fn parse_mail(&mut self) -> Result<(), &'static str> {
        // parse the email and convert it to a internal data structure
        let parsed = mail_parser::Message::parse(&self.buffer)
            .ok_or("Could not parse email using mail_parser")?;
        let message = parsed.try_into()?;

        // clear the message buffer
        self.buffer.clear();

        // send the message to a internal queue
        self.tx
            .send(message)
            .map_err(|_| "Could not send email to own broadcast channel")?;

        Ok(())
    }
}

impl mailin::Handler for MailHandler {
    fn helo(&mut self, _ip: std::net::IpAddr, _domain: &str) -> mailin::Response {
        mailin::response::OK
    }

    fn mail(&mut self, _ip: std::net::IpAddr, _domain: &str, _from: &str) -> mailin::Response {
        mailin::response::OK
    }

    fn rcpt(&mut self, _to: &str) -> mailin::Response {
        mailin::response::OK
    }

    fn data_start(
        &mut self,
        domain: &str,
        from: &str,
        _is8bit: bool,
        _to: &[String],
    ) -> mailin::Response {
        event!(Level::INFO, "New email on {} from {}", domain, from);
        mailin::response::OK
    }

    fn data(&mut self, buf: &[u8]) -> io::Result<()> {
        self.buffer.extend_from_slice(buf);
        Ok(())
    }

    fn data_end(&mut self) -> mailin::Response {
        if let Err(e) = self.parse_mail() {
            event!(Level::WARN, "Error parsing email: {}", e);
        }

        mailin::response::OK
    }

    fn auth_plain(
        &mut self,
        _authorization_id: &str,
        _authentication_id: &str,
        _password: &str,
    ) -> mailin::Response {
        mailin::response::AUTH_OK
    }
}

pub fn smtp_listen<A: ToSocketAddrs>(
    addr: A,
    tx: Sender<MailMessage>,
    enable_tls_auth: bool,
) -> Result<(), mailin_embedded::err::Error> {
    let handler = MailHandler::create(tx);
    let mut server = Server::new(handler);

    let name = env!("CARGO_PKG_NAME");

    // Because mailin-embedded AUTH PLAIN only works over TLS,
    // we need to have a valid SslConfig if auth is enabled.
    // If auth is enabled but the SMTP server does not offer TLS
    // it will returns 503 Bad sequence of commands.
    match enable_tls_auth {
        true => {
            // We generate a self-signed cert on startup
            event!(Level::INFO, "TLS Auth enabled! Generating certificate...");

            let mut cert_params = CertificateParams::default();
            let mut dis_name = DistinguishedName::new();
            dis_name.push(DnType::CommonName, name);
            cert_params.distinguished_name = dis_name;

            let cert =
                Certificate::from_params(cert_params).expect("Cannot generate certificates!");
            let cert_pem = cert
                .serialize_pem()
                .expect("Cannot serialize certificate to PEM format!");

            fs::write("cert.pem", &cert_pem).expect("Cannot write out certificate to a file!");
            fs::write("key.pem", cert.serialize_private_key_pem())
                .expect("Cannot write out key to a file!");

            event!(Level::INFO, "Certificate generated:\n{cert_pem}",);

            let ssl = SslConfig::SelfSigned {
                cert_path: "cert.pem".to_string(),
                key_path: "key.pem".to_string(),
            };

            server
                .with_name(name)
                .with_auth(AuthMechanism::Plain)
                .with_ssl(ssl)?
                .with_addr(addr)?;
        }
        false => {
            server
                .with_name(name)
                .with_ssl(SslConfig::None)?
                .with_addr(addr)?;
        }
    }

    server.serve()?;

    Ok(())
}
